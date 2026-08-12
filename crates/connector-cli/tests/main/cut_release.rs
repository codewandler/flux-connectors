//! `scripts/cut-release.sh` — what a release cut must do, and the three things it must never do.
//!
//! Cutting a release is the one sequence in this repository that ends in something irreversible: a
//! pushed `vX.Y.Z` tag *is* the crates.io publication (AGENTS.md § Publishing contract), and a
//! burned version number cannot be withdrawn. It was nine hand-run steps until C-427. The
//! properties asserted here are the ones that make automating it safe rather than merely shorter:
//!
//! 1. **It regenerates.** This repository is a compiler whose output records the compiler's own
//!    version — every generated `connectors/*.connector.toml` carries
//!    `generator = "flux-connectors <version>"`, and `connectors.lock` hashes them (C-189). A bump
//!    without a regeneration leaves the tree inconsistent with itself, and it surfaces *after* the
//!    commit. Cutting v0.9.0 by hand rewrote 184 artifacts at that step.
//! 2. **It is transactional.** A red gate restores the tree to exactly what it was, so a failed cut
//!    is safe to re-run. Without that, the second run promotes `[Unreleased]` a second time and
//!    mints a phantom version section — the failure flux measured twice before scripting it.
//! 3. **It stages only the release files.** Agents work concurrently here, so another session's
//!    uncommitted or already-staged work must never ride along in a release commit.
//! 4. **It runs every CI gate.** The public site and host operator page are Node suites, so a Rust-
//!    only preflight can tag and publish while a required consumer surface is red (C-453).
//! 5. **It pushes nothing and publishes nothing.**
//!
//! # Why a scratch repository and a stub `cargo`
//!
//! A test that cut a real release would be a release. So each test builds a miniature of this
//! repository's *shape* — the manifest sections the script edits, both changelogs, one generated
//! artifact of each kind — inside its own git repository under the build tree, and puts a stub
//! `cargo` first on `PATH`.
//!
//! The stub is not a mock of convenience: it models the two contracts the script actually depends
//! on, and nothing else.
//!
//!   - `build` stamps the version it reads out of `Cargo.toml` into every artifact, which is what
//!     `crates/connector-cli/src/seam.rs`'s `generator()` does by way of `CARGO_PKG_VERSION`;
//!   - `diff` reports drift by **printing** it and exits 0 either way, which is what
//!     `show_diff` in `src/lib.rs` does — and is why the script reads its text rather than its
//!     status. A stub that failed instead would let a `set -e`-only check pass this file while
//!     passing through the exact defect the script exists to catch.
//!
//! `git` is the real `git` throughout: the transactional and staging properties are properties of
//! git's index and working tree, and a stub of those would assert nothing.
//!
//! The publish path is deliberately not modelled at all — the stub *fails* on `cargo publish`, so
//! "the cut never publishes" is a property of the run rather than of a grep over the source.
//!
//! # One real thing this file needs on the machine: `cargo-nextest`
//!
//! Since C-543 the gate runs the suite through `cargo nextest run`, and the script refuses to cut
//! on a machine without the runner — with the install line, rather than with cargo's bare
//! `no such command: nextest`. That preflight asks `command -v cargo-nextest`, which reaches the
//! **real** binary on `PATH` and not the stub, because the question is whether the runner is
//! installed and the stub cannot answer it honestly. So every green-cut test here needs
//! `cargo-nextest` present; without it they fail at the preflight, which is the same refusal a
//! developer would get and names its own fix:
//!
//! ```text
//! cargo install cargo-nextest --locked --version 0.9.143
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common::Fixture;

/// The version every scratch repository starts on, matching the shape of a real pre-1.0 line.
const FROM: &str = "0.9.0";
/// The version the tests cut to. `minor` from `0.9.0`, so it also exercises the breaking-signal rule.
const TO: &str = "0.10.0";
/// The complete promoted customer section, including Markdown headings that git's default tag
/// cleanup treats as comments and would otherwise remove.
const CUSTOMER_BODY: &str = "\
### Action needed

- Stop every old process before upgrading.

### New

- You can now do a thing you could not do before.

### Fixed

- Installation examples now name the published packages.
";

const PUBLIC_READMES: &[(&str, &str)] = &[
    (
        "crates/connector-address/README.md",
        "codewandler-connector-address",
    ),
    ("crates/catalog/README.md", "codewandler-connector-catalog"),
    (
        "crates/catalog-reader/README.md",
        "codewandler-connector-catalog-reader",
    ),
    (
        "crates/connector-resolve/README.md",
        "codewandler-connector-resolve",
    ),
    (
        "crates/connector-secrets/README.md",
        "codewandler-connector-secrets",
    ),
    (
        "crates/connector-pack/README.md",
        "codewandler-connector-pack",
    ),
];

// ---------------------------------------------------------------------------------------------
// The scratch repository
// ---------------------------------------------------------------------------------------------

/// A miniature of this repository inside its own git repo, plus a stub `cargo` on `PATH`.
struct Scratch {
    fixture: Fixture,
}

impl Scratch {
    /// Build the tree, `git init` it, and commit everything.
    fn new(label: &str) -> Self {
        let fixture = Fixture::new(label);
        let scratch = Self { fixture };
        scratch.write_tree();
        scratch.write_stub_cargo();
        scratch.write_stub_npm();
        scratch.init_git();
        scratch
    }

    fn root(&self) -> PathBuf {
        self.fixture.root().join("repo")
    }

    /// The stub's invocation log, kept *outside* the repository so it never shows up in
    /// `git status` — the transactional test compares that output byte for byte.
    fn cargo_log_path(&self) -> PathBuf {
        self.fixture.root().join("cargo-invocations.log")
    }

    /// As [`Scratch::cargo_log_path`], for the two Node gates (C-453).
    fn npm_log_path(&self) -> PathBuf {
        self.fixture.root().join("npm-invocations.log")
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root().join(relative);
        fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create parent");
        fs::write(&path, contents).expect("write scratch file");
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root().join(relative)).expect("read scratch file")
    }

    fn write_tree(&self) {
        // The manifest sections the script edits. `serde` is here as a third-party requirement that
        // must survive untouched: the internal requirements are found by `path = "crates/`, not by
        // matching the version string everywhere it appears.
        self.write(
            "Cargo.toml",
            &format!(
                r#"[workspace]
members = ["crates/catalog"]

[workspace.package]
edition = "2021"
version = "{FROM}"
rust-version = "1.87"

[workspace.dependencies]
connector-spec = {{ package = "codewandler-connector-spec", version = "{FROM}", path = "crates/connector-spec" }}
catalog = {{ package = "codewandler-connector-catalog", version = "{FROM}", path = "crates/catalog" }}
serde = {{ version = "1", features = ["derive"] }}
"#
            ),
        );
        self.write(
            "Cargo.lock",
            &format!(
                "version = 4\n\n[[package]]\nname = \"connector-cli\"\nversion = \"{FROM}\"\n"
            ),
        );
        // `v0.9.0` is rewritten; the flux-lang line is not — a bare version in prose could belong to
        // anything, so the script only ever rewrites the `v`-prefixed form.
        self.write(
            "README.md",
            &format!(
                "# flux-connectors\n\n> **v{FROM} is a compiler, a catalogue and a Tool pack.**\n\nPinned to flux-lang 0.46.\n"
            ),
        );
        for (path, package) in PUBLIC_READMES {
            let mut contents = format!(
                "# {package}\n\n```toml\n{package} = \"{}\"\n```\n\nCompatible with flux-core 0.46.\n",
                release_line(FROM)
            );
            if *package == "codewandler-connector-secrets" {
                contents.push_str(&format!(
                    "\n```toml\n# {package} = {{ version = \"{}\", features = [\"vault\"] }}\n```\n",
                    release_line(FROM)
                ));
            }
            self.write(path, &contents);
        }
        self.write(
            "CHANGELOG.md",
            &format!(
                "# Changelog\n\n## [Unreleased]\n\n### Changed\n\n- Something an implementor needs to find later.\n\n## [{FROM}] — 2026-08-01\n\n### Added\n\n- The previous release.\n"
            ),
        );
        self.write(
            "WHATS-NEW.md",
            &format!(
                "# What's new in flux-connectors\n\n## [Unreleased]\n\n{CUSTOMER_BODY}\n## [{FROM}] — 2026-08-01\n\n### New\n\n- The previous release.\n"
            ),
        );

        // One generated artifact of each kind that carries the generator string, at the paths
        // AGENTS.md § Source and generated-file boundaries names.
        for path in GENERATOR_STAMPED {
            self.write(path, &stamped(path, FROM));
        }
        // Generated, but carrying no version of its own — it still has to be inside the snapshot.
        self.write(
            "crates/catalog/ops/acme/acme-thing-get.flux",
            "op get() {}\n",
        );
        self.write(
            "assets/readme-snippet-light.svg",
            "<svg><!-- generated --></svg>\n",
        );
        // Hand-authored, and the source of truth for the README image: adjacent to a generated
        // artifact and deliberately not one.
        self.write("assets/readme-snippet.flux", "op get() {}\n");

        // A concurrent session's territory: source, never rewritten or committed by a cut.
        self.write("providers/acme.toml", "id = \"acme\"\n");
        self.write("specs/acme/v1.json", "{\"openapi\":\"3.0.3\"}\n");
    }

    /// A stub `cargo` modelling exactly two contracts — see this file's header for why each one.
    fn write_stub_cargo(&self) {
        let bin = self.fixture.root().join("bin");
        fs::create_dir_all(&bin).expect("create stub bin");
        let artifacts = GENERATOR_STAMPED.join(" ");
        let stub = format!(
            r#"#!/usr/bin/env bash
# Stub `cargo`, first on PATH for the cut-release tests. Run from the scratch repo root.
set -uo pipefail
printf '%s\n' "$*" >>"$STUB_LOG"

workspace_version() {{
  awk '
    /^\[workspace\.package\]/ {{ s = 1; next }}
    /^\[/                     {{ s = 0 }}
    s && /^version[[:space:]]*=/ {{ print; exit }}
  ' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/'
}}

ARTIFACTS=({artifacts})

case "${{1:-}}" in
  run)
    version=$(workspace_version)
    case "$*" in
      *" build"*)
        # What `seam::generator()` does: stamp the compiler's own version into every artifact.
        for f in "${{ARTIFACTS[@]}}"; do
          sed -i -E "s/flux-connectors [0-9]+\.[0-9]+\.[0-9]+/flux-connectors $version/g" "$f"
        done
        echo "1 provider, ${{#ARTIFACTS[@]}} artifacts; ${{#ARTIFACTS[@]}} written"
        ;;
      *" diff"*)
        drift=0
        for f in "${{ARTIFACTS[@]}}"; do
          grep -q "flux-connectors $version" "$f" || drift=$((drift + 1))
        done
        if [ "$drift" -eq 0 ]; then
          echo "${{#ARTIFACTS[@]}} artifacts up to date (1 provider checked)"
        else
          echo "$drift artifacts would change (1 provider checked)"
        fi
        # Exit 0 either way — `show_diff` is a preview and reports drift by printing it.
        ;;
      *) echo "stub cargo: unexpected run: $*" >&2; exit 90 ;;
    esac
    ;;
  update)
    sed -i -E "s/^version = \"[0-9]+\.[0-9]+\.[0-9]+\"/version = \"$(workspace_version)\"/" Cargo.lock
    ;;
  fmt | build | test | clippy | nextest)
    # `nextest` joins the gate verbs in C-543: the suite runs through `cargo nextest run`, and
    # doc-tests stay behind `cargo test --workspace --doc`, so a cut invokes both spellings.
    #
    # A gate step is a *direct* child of the script, so $PPID is the script itself — which is what
    # makes "a fatal signal mid-cut" reproducible at a known point rather than by racing a pipe.
    # The signal rides on `nextest` rather than `test`, so it still lands on the FIRST gate step
    # that runs tests; keying it on `test` would now fire at the doc-test step instead, one step
    # later and for no stated reason.
    if [ -n "${{STUB_SIGNAL:-}}" ] && [ "$1" = "nextest" ]; then
      kill -"$STUB_SIGNAL" "$PPID"
      exit 0
    fi
    if [ "${{STUB_GATE:-green}}" = "red" ]; then
      echo "error: the stub gate is red" >&2
      exit 101
    fi
    ;;
  publish)
    echo "stub cargo: a release cut must never publish" >&2
    exit 99
    ;;
  *) echo "stub cargo: unexpected invocation: $*" >&2; exit 90 ;;
esac
exit 0
"#
        );
        let path = bin.join("cargo");
        fs::write(&path, stub).expect("write stub cargo");
        make_executable(&path);
    }

    /// A stub `npm` that records which package tree and command the release gate selected.
    fn write_stub_npm(&self) {
        let bin = self.fixture.root().join("bin");
        fs::create_dir_all(&bin).expect("create stub bin");
        let stub = r#"#!/usr/bin/env bash
set -uo pipefail
printf '%s\n' "$*" >>"$STUB_NPM_LOG"
if [ "${STUB_NPM_GATE:-green}" = "red" ]; then
  echo "error: the stub Node gate is red" >&2
  exit 102
fi
exit 0
"#;
        let path = bin.join("npm");
        fs::write(&path, stub).expect("write stub npm");
        make_executable(&path);
    }

    fn init_git(&self) {
        let root = self.root();
        self.git(&["-c", "init.defaultBranch=main", "init", "-q"]);
        self.git(&["config", "user.email", "cut-release@example.invalid"]);
        self.git(&["config", "user.name", "cut-release test"]);
        self.git(&["config", "commit.gpgsign", "false"]);
        self.git(&["config", "tag.gpgsign", "false"]);
        // A developer's global hooks directory must not run inside a fixture.
        let hooks = self.fixture.root().join("no-hooks");
        fs::create_dir_all(&hooks).expect("create empty hooks dir");
        self.git(&["config", "core.hooksPath", &hooks.display().to_string()]);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "Initial state"]);

        // The one assertion that must hold before anything runs the script: `cut-release.sh` starts
        // with `cd "$(git rev-parse --show-toplevel)"`, so a scratch repo that failed to initialise
        // would send it up into the real worktree and cut a release there.
        let toplevel = self.git(&["rev-parse", "--show-toplevel"]);
        let expected = fs::canonicalize(&root).expect("canonical scratch root");
        let actual = fs::canonicalize(toplevel.trim()).expect("canonical toplevel");
        assert_eq!(
            actual,
            expected,
            "the scratch repository is not its own git toplevel — the script would escape into {}",
            actual.display()
        );
    }

    /// Run `git` in the scratch repo, asserting success, returning stdout.
    fn git(&self, args: &[&str]) -> String {
        let output = self.command("git", args).output().expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git speaks utf-8")
    }

    /// A command in the scratch repo, with the stub `cargo` first on `PATH` and git fenced in.
    fn command(&self, program: &str, args: &[&str]) -> Command {
        let mut command = Command::new(program);
        let path = format!(
            "{}:{}",
            self.fixture.root().join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        command
            .args(args)
            .current_dir(self.root())
            .env("PATH", path)
            // `HOME` so a developer's global gitconfig (a signing key, a commit template) cannot
            // reach in; `GIT_CEILING_DIRECTORIES` so no git invocation can walk out of the fixture.
            .env("HOME", self.fixture.root())
            .env("GIT_CEILING_DIRECTORIES", self.fixture.root())
            .env("STUB_LOG", self.cargo_log_path())
            .env("STUB_NPM_LOG", self.npm_log_path())
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE");
        command
    }

    /// Run `scripts/cut-release.sh` against the scratch repo.
    fn cut(&self, args: &[&str]) -> Output {
        self.cut_with_env(&[], args)
    }

    /// As [`Scratch::cut`], with extra environment the stub `cargo` reads to misbehave on purpose.
    fn cut_with_env(&self, env: &[(&str, &str)], args: &[&str]) -> Output {
        let script = workspace_root().join("scripts/cut-release.sh");
        assert!(
            script.is_file(),
            "scripts/cut-release.sh does not exist at {}",
            script.display()
        );
        let mut command = self.command("bash", &[]);
        command.arg(&script).args(args);
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().expect("run cut-release.sh")
    }

    fn cargo_invocations(&self) -> String {
        fs::read_to_string(self.cargo_log_path()).unwrap_or_default()
    }

    fn npm_invocations(&self) -> String {
        fs::read_to_string(self.npm_log_path()).unwrap_or_default()
    }

    fn status(&self) -> String {
        self.git(&["status", "--porcelain"])
    }

    fn log(&self) -> String {
        self.git(&["log", "--format=%s"])
    }

    fn tags(&self) -> String {
        self.git(&["tag", "--list"])
    }

    /// Every file under the repo except git's own bookkeeping, mapped to its exact contents.
    fn snapshot(&self) -> Snapshot {
        let mut files = BTreeMap::new();
        collect(&self.root(), &self.root(), &mut files);
        files
    }
}

/// Every scratch file is text, so a snapshot holds `String`: a mismatch between two `Vec<u8>` maps
/// prints as several thousand decimal byte values, which is a test that reports its failure in a
/// form nobody reads.
type Snapshot = BTreeMap<PathBuf, String>;

/// Assert the tree is exactly as it was, naming the files that differ and showing them.
#[track_caller]
fn assert_tree_unchanged(before: &Snapshot, after: &Snapshot, context: &str) {
    let mut differences = String::new();
    for (path, was) in before {
        match after.get(path) {
            Some(now) if now == was => {}
            Some(now) => differences.push_str(&format!(
                "\n--- {} was ---\n{was}\n--- {} is now ---\n{now}",
                path.display(),
                path.display()
            )),
            None => differences.push_str(&format!("\n--- {} was removed ---", path.display())),
        }
    }
    for path in after.keys() {
        if !before.contains_key(path) {
            differences.push_str(&format!("\n--- {} was created ---", path.display()));
        }
    }
    assert!(differences.is_empty(), "{context}{differences}");
}

/// The generated artifacts that carry the generator string, one of each kind the boundaries table
/// names. The stub stamps exactly these; the script finds them by walking the same directories.
const GENERATOR_STAMPED: &[&str] = &[
    "connectors/acme.connector.toml",
    "connectors.lock",
    // The canonical document (C-536) and the pack compiled from it (C-537), which the release
    // paths must carry or a cut leaves its own regeneration uncommitted.
    "catalog/acme.catalog.json",
    "crates/catalog-reader/catalog.pack",
    "crates/catalog/src/generated.rs",
    "crates/catalog/src/generated/acme.rs",
    "web/public/catalog.json",
];

fn stamped(path: &str, version: &str) -> String {
    if path.ends_with(".json") {
        format!("{{\"generator\":\"flux-connectors {version}\"}}\n")
    } else if path.ends_with(".rs") {
        format!("//! Generated by flux-connectors {version} — do not edit.\n")
    } else {
        format!(
            "# Generated by flux-connectors {version} — do not edit.\ngenerator = \"flux-connectors {version}\"\n"
        )
    }
}

fn release_line(version: &str) -> String {
    let mut components = version.split('.');
    let major = components.next().expect("version has a major component");
    let minor = components.next().expect("version has a minor component");
    format!("{major}.{minor}")
}

fn collect(root: &Path, dir: &Path, into: &mut Snapshot) {
    for entry in fs::read_dir(dir).expect("read scratch dir") {
        let path = entry.expect("scratch dir entry").path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, into);
        } else {
            let relative = path.strip_prefix(root).expect("relative").to_path_buf();
            let contents = fs::read(&path).expect("read scratch file");
            into.insert(relative, String::from_utf8_lossy(&contents).into_owned());
        }
    }
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).expect("stat stub").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod stub");
}

/// The repository root: `crates/connector-cli/tests/..` twice up from `CARGO_MANIFEST_DIR`.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/connector-cli sits two levels below the workspace root")
        .to_path_buf()
}

fn describe(output: &Output) -> String {
    format!(
        "status: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

// ---------------------------------------------------------------------------------------------
// The properties
// ---------------------------------------------------------------------------------------------

/// A red gate leaves the tree byte-for-byte as it found it, and leaves no commit and no tag.
///
/// This is the property that makes a failed cut safe to re-run. The gate runs at the *end*, after
/// both changelogs are promoted and every version string is bumped, because it has to test what is
/// about to be tagged — so a red gate is exactly when a half-cut tree would otherwise be left
/// behind, and a second run would promote `[Unreleased]` again into a phantom version section.
///
/// The comparison is deliberately whole-tree bytes *and* `git status --porcelain`, not one or the
/// other: bytes catch a restored-but-rewritten file, and porcelain catches an artifact the cut
/// created that a byte comparison of the *original* files would never look at.
#[test]
fn a_red_gate_restores_the_tree_exactly() {
    let scratch = Scratch::new("cut-release-red-gate");

    // Uncommitted work of three kinds, which a restore has to preserve exactly as it found it.
    scratch.write("providers/acme.toml", "id = \"acme\"\nvendor = \"Acme\"\n");
    scratch.write("providers/inflight.toml", "id = \"inflight\"\n");
    scratch.write("specs/acme/v1.json", "{\"openapi\":\"3.1.0\"}\n");
    scratch.git(&["add", "specs/acme/v1.json"]);

    let status_before = scratch.status();
    let tree_before = scratch.snapshot();
    let log_before = scratch.log();

    let output = scratch.cut_with_env(&[("STUB_GATE", "red")], &[TO]);

    assert!(
        !output.status.success(),
        "a red gate must fail the cut\n{}",
        describe(&output)
    );
    assert_eq!(
        scratch.status(),
        status_before,
        "the working tree is not what it was before the cut\n{}",
        describe(&output)
    );
    assert_tree_unchanged(
        &tree_before,
        &scratch.snapshot(),
        &format!("a red gate did not restore the tree\n{}", describe(&output)),
    );
    assert_eq!(scratch.log(), log_before, "a failed cut committed");
    assert_eq!(scratch.tags(), "", "a failed cut tagged");

    // Restoring is not enough on its own: the second run has to succeed, which is the whole point.
    let second = scratch.cut(&[TO]);
    assert!(
        second.status.success(),
        "a cut re-run after a red gate must succeed\n{}",
        describe(&second)
    );
    assert_eq!(
        scratch.read("CHANGELOG.md").matches("## [0.10.0]").count(),
        1,
        "the re-run promoted [Unreleased] twice — the phantom version section"
    );
}

/// Both Node suites are release gates, not post-publication diagnostics (C-453).
#[test]
fn a_cut_runs_both_node_gates_before_it_tags() {
    let scratch = Scratch::new("cut-release-node-gates");

    let output = scratch.cut(&[TO]);
    assert!(
        output.status.success(),
        "the cut failed\n{}",
        describe(&output)
    );
    assert_eq!(
        scratch.npm_invocations().lines().collect::<Vec<_>>(),
        [
            "--prefix web ci",
            "--prefix web run build",
            "--prefix web test",
            "--prefix crates/connectors-api/ui ci",
            "--prefix crates/connectors-api/ui test",
        ],
        "the release preflight did not run the same two Node gates as CI"
    );
    assert!(
        scratch.tags().contains(&format!("v{TO}")),
        "the green cut did not reach its tag"
    );
}

/// A red Node suite has the same transactional guarantee as a red Rust gate.
#[test]
fn a_red_node_gate_restores_the_tree_and_creates_no_tag() {
    let scratch = Scratch::new("cut-release-red-node-gate");
    let status_before = scratch.status();
    let tree_before = scratch.snapshot();
    let log_before = scratch.log();

    let output = scratch.cut_with_env(&[("STUB_NPM_GATE", "red")], &[TO]);

    assert!(
        !output.status.success(),
        "a red Node gate must fail the cut\n{}",
        describe(&output)
    );
    assert_eq!(
        scratch.status(),
        status_before,
        "the index was not restored"
    );
    assert_tree_unchanged(
        &tree_before,
        &scratch.snapshot(),
        &format!(
            "a red Node gate did not restore the tree\n{}",
            describe(&output)
        ),
    );
    assert_eq!(scratch.log(), log_before, "a red Node gate committed");
    assert_eq!(scratch.tags(), "", "a red Node gate tagged");
}

/// The promotable section must be first; otherwise a cut appends a new release below newer ones.
#[test]
fn a_cut_refuses_a_displaced_unreleased_section() {
    let scratch = Scratch::new("cut-release-changelog-order");
    let whats_new = scratch.read("WHATS-NEW.md").replacen(
        "## [Unreleased]",
        "## 0.9.1\n\n### Fixed\n\n- Already released.\n\n## [Unreleased]",
        1,
    );
    scratch.write("WHATS-NEW.md", &whats_new);
    let tree_before = scratch.snapshot();

    let output = scratch.cut(&[TO]);

    assert!(
        !output.status.success(),
        "a displaced [Unreleased] section must refuse the cut\n{}",
        describe(&output)
    );
    assert_tree_unchanged(
        &tree_before,
        &scratch.snapshot(),
        "the changelog-order refusal changed the tree",
    );
    assert_eq!(scratch.tags(), "", "a malformed customer changelog tagged");
}

/// A fatal signal mid-cut restores the tree too, and this is a measured hole rather than a
/// hypothetical one.
///
/// Piping the script's stdout into `head` while developing it killed it with SIGPIPE partway
/// through the bump. **An EXIT trap alone does not run for an untrapped fatal signal**, so the
/// snapshot armed one line earlier restored nothing: the tree was left with both changelogs
/// promoted and the manifest bumped — exactly the half-cut state the transaction exists to prevent,
/// reached through the one exit path it did not cover.
///
/// SIGTERM is here beside it because the signal is not the point: an interrupted cut is a cut that
/// leaves the tree alone, whichever way it is interrupted.
#[test]
fn a_fatal_signal_mid_cut_restores_the_tree_too() {
    for signal in ["PIPE", "TERM"] {
        let scratch = Scratch::new(&format!("cut-release-signal-{}", signal.to_lowercase()));
        let status_before = scratch.status();
        let tree_before = scratch.snapshot();

        // Delivered from inside a gate step, so it lands after both changelogs are promoted, every
        // version string is bumped and every artifact is regenerated — the worst possible moment.
        let output = scratch.cut_with_env(&[("STUB_SIGNAL", signal)], &[TO]);

        assert!(
            !output.status.success(),
            "a cut killed by SIG{signal} must not report success\n{}",
            describe(&output)
        );
        assert_tree_unchanged(
            &tree_before,
            &scratch.snapshot(),
            &format!("SIG{signal} left a half-cut tree\n{}", describe(&output)),
        );
        assert_eq!(
            scratch.status(),
            status_before,
            "SIG{signal}: {}",
            describe(&output)
        );
        assert_eq!(scratch.log(), "Initial state\n", "SIG{signal} committed");
        assert_eq!(scratch.tags(), "", "SIG{signal} tagged");
    }
}

/// A cut regenerates every artifact, so `diff` is clean and every generator string carries the new
/// version — the failure this script exists to prevent, caught here rather than by CI after the fact.
#[test]
fn a_cut_regenerates_every_artifact_and_stamps_the_new_version() {
    let scratch = Scratch::new("cut-release-regenerates");

    let output = scratch.cut(&["minor"]);
    assert!(
        output.status.success(),
        "the cut failed\n{}",
        describe(&output)
    );

    // `diff`, run against the tree the cut committed, reports everything up to date.
    let diff = scratch
        .command("cargo", &["run", "-p", "connector-cli", "--", "diff"])
        .output()
        .expect("run stub diff");
    let diff = String::from_utf8_lossy(&diff.stdout).to_string();
    assert!(
        diff.contains("up to date"),
        "after a cut, `diff` still reports drift: {diff}"
    );

    for path in GENERATOR_STAMPED {
        let contents = scratch.read(path);
        assert!(
            contents.contains(&format!("flux-connectors {TO}")),
            "{path} does not carry the new generator string: {contents}"
        );
        assert!(
            !contents.contains(&format!("flux-connectors {FROM}")),
            "{path} still carries the old generator string: {contents}"
        );
    }

    // Regeneration happened after the bump and before the gate — the ordering is the property, and
    // a build that ran first would have stamped the old version into every artifact.
    let invocations: Vec<String> = scratch
        .cargo_invocations()
        .lines()
        .map(str::to_owned)
        .collect();
    let build = invocations
        .iter()
        .position(|line| line.contains("connector-cli -- build"))
        .expect("the cut regenerated");
    // Pinned to the runner the gate actually names (C-543). This was `test --workspace`, which a
    // nextest gate still satisfies by prefix through its `test --workspace --doc` step — so it
    // would have kept passing while asserting about a step this comment never meant, which is the
    // drift the assertion exists to catch wearing the assertion's own clothes.
    let gate = invocations
        .iter()
        .position(|line| line.starts_with("nextest run --workspace"))
        .expect("the cut ran the gate");
    assert!(
        build < gate,
        "the gate ran before the regeneration: {invocations:?}"
    );
}

/// Every place the version reaches, bumped together, and nothing that merely looks like a version.
#[test]
fn a_cut_bumps_the_manifest_the_lock_and_the_readme() {
    let scratch = Scratch::new("cut-release-bumps");

    let output = scratch.cut(&["minor"]);
    assert!(
        output.status.success(),
        "the cut failed\n{}",
        describe(&output)
    );

    let manifest = scratch.read("Cargo.toml");
    assert!(
        manifest.contains(&format!("version = \"{TO}\"\nrust-version")),
        "[workspace.package].version was not bumped: {manifest}"
    );
    // Both internal path-dependency requirements move; they are found by `path = "crates/`, so a
    // crate added later is bumped without an edit to the script.
    assert_eq!(
        manifest
            .lines()
            .filter(|line| line.contains("path = \"crates/")
                && line.contains(&format!("version = \"{TO}\"")))
            .count(),
        2,
        "the internal path-dependency requirements were not bumped: {manifest}"
    );
    assert!(
        !manifest.contains(FROM),
        "something still asks for {FROM}: {manifest}"
    );
    assert!(
        manifest.contains("serde = { version = \"1\""),
        "a third-party requirement was rewritten: {manifest}"
    );

    assert!(
        scratch
            .read("Cargo.lock")
            .contains(&format!("version = \"{TO}\"")),
        "the lockfile was not re-locked"
    );

    let readme = scratch.read("README.md");
    assert!(readme.contains(&format!("v{TO}")), "README.md: {readme}");
    assert!(
        readme.contains("flux-lang 0.46"),
        "a bare version in prose was rewritten: {readme}"
    );

    for (path, package) in PUBLIC_READMES {
        let readme = scratch.read(path);
        let expected = format!("{package} = \"{}\"", release_line(TO));
        assert!(
            readme.lines().any(|line| line == expected),
            "{path} did not move its dependency example to the release line: {readme}"
        );
        let stale = format!("{package} = \"{}\"", release_line(FROM));
        assert!(
            !readme.lines().any(|line| line == stale),
            "{path} retained its old dependency example: {readme}"
        );
        assert!(
            readme.contains("flux-core 0.46"),
            "{path} rewrote an unrelated version in prose: {readme}"
        );
    }
    let secrets = scratch.read("crates/connector-secrets/README.md");
    assert!(
        secrets.contains(&format!(
            "# codewandler-connector-secrets = {{ version = \"{}\", features = [\"vault\"] }}",
            release_line(TO)
        )),
        "the commented Vault feature example was not bumped: {secrets}"
    );
    assert!(
        !secrets.contains(&format!(
            "# codewandler-connector-secrets = {{ version = \"{}\", features = [\"vault\"] }}",
            release_line(FROM)
        )),
        "the commented Vault feature example retained its old release line: {secrets}"
    );

    // Both changelogs promoted, each keeping a fresh empty [Unreleased] above the new section.
    for file in ["CHANGELOG.md", "WHATS-NEW.md"] {
        let contents = scratch.read(file);
        assert!(
            contents.contains("## [Unreleased]") && contents.contains(&format!("## [{TO}] — ")),
            "{file} was not promoted: {contents}"
        );
    }

    let log = scratch.log();
    assert_eq!(
        log.lines().next(),
        Some(format!("Release v{TO}").as_str()),
        "the release commit is not the shape previous ones are"
    );
    // Annotated, not lightweight: `git push --follow-tags` only pushes annotated tags, so a
    // lightweight one sits there looking cut while no workflow ever fires.
    assert_eq!(
        scratch.git(&["cat-file", "-t", &format!("v{TO}")]).trim(),
        "tag",
        "the tag is lightweight"
    );
    let tag = scratch.git(&["cat-file", "-p", &format!("v{TO}")]);
    let (_, message) = tag
        .split_once("\n\n")
        .expect("an annotated tag has headers and a message");
    assert_eq!(
        message,
        format!("flux-connectors {TO}\n\n{CUSTOMER_BODY}"),
        "the annotated tag did not retain the complete promoted customer section"
    );
}

/// A concurrent session's work — uncommitted or already staged — never rides along in the commit.
#[test]
fn a_cut_stages_only_the_release_files() {
    let scratch = Scratch::new("cut-release-staging");

    // Another session, mid-story: one file edited, one already `git add`ed into the shared index.
    scratch.write("providers/acme.toml", "id = \"acme\"\nvendor = \"Acme\"\n");
    scratch.write("specs/acme/v1.json", "{\"openapi\":\"3.1.0\"}\n");
    scratch.git(&["add", "specs/acme/v1.json"]);
    let status_before = scratch.status();

    let output = scratch.cut(&[TO]);
    assert!(
        output.status.success(),
        "the cut failed\n{}",
        describe(&output)
    );

    let committed = scratch.git(&["show", "--name-only", "--format=", "HEAD"]);
    let swept: Vec<&str> = committed
        .lines()
        .filter(|line| line.starts_with("providers/") || line.starts_with("specs/"))
        .collect();
    assert!(
        swept.is_empty(),
        "the release commit swept in another session's work: {swept:?}"
    );
    assert_eq!(
        scratch.status(),
        status_before,
        "another session's working tree and index did not survive the cut"
    );
}

/// A cut is taken on top of committed work, so a dirty generated tree is refused before anything is
/// touched. Those artifacts are whole-catalogue and coordinator-owned: a cut regenerates them
/// whole, and cannot tell somebody else's uncommitted regeneration apart from its own.
///
/// Both kinds are refused, and the untracked one is the harder half: it is invisible to
/// `git diff HEAD -- connectors`, and step 8's `git commit --only connectors` would commit it.
#[test]
fn a_cut_refuses_when_a_generated_artifact_is_already_dirty() {
    let scratch = Scratch::new("cut-release-dirty");
    scratch.write(
        "connectors/acme.connector.toml",
        &format!(
            "{}# edited by another session\n",
            stamped("connectors/acme.connector.toml", FROM)
        ),
    );
    scratch.write(
        "connectors/ghost.connector.toml",
        &stamped("connectors/ghost.connector.toml", FROM),
    );
    let tree_before = scratch.snapshot();

    let output = scratch.cut(&[TO]);

    assert!(
        !output.status.success(),
        "a dirty generated tree must refuse the cut\n{}",
        describe(&output)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for named in [
        "connectors/acme.connector.toml",
        "connectors/ghost.connector.toml",
    ] {
        assert!(
            stderr.contains(named),
            "the refusal does not name {named}: {stderr}"
        );
    }
    assert_tree_unchanged(
        &tree_before,
        &scratch.snapshot(),
        "the refusal was not free — something was already written",
    );
    assert_eq!(scratch.tags(), "", "a refused cut tagged");
}

/// The cut prepares the irreversible step and does not take it: nothing is pushed, and `cargo
/// publish` is never invoked in any form.
///
/// Both halves are properties of the *run* rather than of a grep over the script's text, which would
/// have to tell an executed `git push` apart from the one the script prints as an instruction.
#[test]
fn a_cut_pushes_nothing_and_publishes_nothing() {
    let scratch = Scratch::new("cut-release-no-push");

    let remote = scratch.fixture.root().join("origin.git");
    let init = Command::new("git")
        .args(["init", "--bare", "-q", "--initial-branch=main"])
        .arg(&remote)
        .output()
        .expect("init bare remote");
    assert!(init.status.success(), "could not create the bare remote");
    scratch.git(&["remote", "add", "origin", &remote.display().to_string()]);

    let output = scratch.cut(&[TO]);
    assert!(
        output.status.success(),
        "the cut failed\n{}",
        describe(&output)
    );

    let refs = Command::new("git")
        .args(["--git-dir"])
        .arg(&remote)
        .args(["for-each-ref"])
        .output()
        .expect("read remote refs");
    assert_eq!(
        String::from_utf8_lossy(&refs.stdout).trim(),
        "",
        "the cut pushed to origin"
    );

    let invocations = scratch.cargo_invocations();
    assert!(
        !invocations.lines().any(|line| line.starts_with("publish")),
        "the cut invoked `cargo publish`: {invocations}"
    );

    // And the tag it did create is still purely local, waiting for a decision.
    assert!(
        scratch.tags().contains(&format!("v{TO}")),
        "the cut did not tag: {}",
        scratch.tags()
    );
}

/// The bump rule: `patch` and `minor` both work, and pre-1.0 the **minor** position is the breaking
/// signal — so `major` is refused rather than quietly reaching 1.0.0 from a keyword.
#[test]
fn the_bump_keywords_follow_this_repositorys_pre_1_0_rule() {
    let minor = Scratch::new("cut-release-minor");
    let output = minor.cut(&["minor"]);
    assert!(
        output.status.success(),
        "minor failed\n{}",
        describe(&output)
    );
    assert!(
        minor.read("Cargo.toml").contains("version = \"0.10.0\""),
        "minor did not reset the patch component"
    );

    let patch = Scratch::new("cut-release-patch");
    let output = patch.cut(&["patch"]);
    assert!(
        output.status.success(),
        "patch failed\n{}",
        describe(&output)
    );
    assert!(
        patch.read("Cargo.toml").contains("version = \"0.9.1\""),
        "patch did not bump the patch component"
    );

    let major = Scratch::new("cut-release-major");
    let tree_before = major.snapshot();
    let output = major.cut(&["major"]);
    assert!(
        !output.status.success(),
        "pre-1.0, `major` must be refused\n{}",
        describe(&output)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("1.0.0"),
        "the refusal does not say how to declare 1.0.0 deliberately: {stderr}"
    );
    assert_tree_unchanged(&tree_before, &major.snapshot(), "the refusal was not free");
}
