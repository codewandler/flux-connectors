//! The declared minimum supported Rust version is a checked claim, not a number in a manifest.
//!
//! `[workspace.package] rust-version` is inherited by every crate here, four of which are
//! **published**. A published crate whose declared MSRV is below what it actually compiles under is
//! a broken promise to downstream consumers, and it is fixable only in a later version — so the
//! number has to be true, and something has to say so before a tag is pushed.
//!
//! Until C-213 nothing did. A caret requirement `jsonwebtoken = "10.3"` resolved to 10.4.0, which
//! declares `rust-version = 1.88.0` against this workspace's 1.87, and the incompatibility surfaced
//! only because a person read the lockfile. `resolver = "2"` performs no MSRV-aware version
//! selection, so cargo picked the newest semver-compatible release and said nothing.
//!
//! # Which graph this reads, and why
//!
//! [`dependency_fence.rs`](../dependency_fence.rs) reads `Cargo.lock` because its question is "is
//! this crate here at all", and an edge hidden behind a feature flag is exactly the one worth
//! catching. **This question is different: an MSRV is a property of what is *compiled*.** A crate
//! sitting in the lock behind an off-by-default feature is never handed to rustc, so it cannot
//! break a build on any toolchain, and a fence reading the lock would be red on a tree that is
//! correct.
//!
//! So this reads the graph cargo itself resolves with features applied —
//! `cargo metadata --locked --offline`, the same instrument
//! [`pack_links_no_http_client.rs`](../pack_links_no_http_client.rs) uses. **What it must never be
//! is a hand-kept list of dependency versions**: that would be the same defect one level up, and
//! `docs/stories/C-81-declared-counts-are-checked.md` is this repository's standing example of what
//! hand-maintained numbers do.
//!
//! # What this does *not* cover
//!
//! 1. **Whether the code compiles on the declared MSRV.** This reads *declarations*. A dependency
//!    that uses a 1.88 language feature without raising its own `rust-version`, or a `let`-chain
//!    written in this repository's own `src/`, passes here untouched. Only a build on the declared
//!    toolchain proves that, and CI does not run one — see
//!    [`ci_pins_a_toolchain_far_above_the_declared_msrv`], which states the gap rather than
//!    implying coverage that does not exist.
//! 2. **Dev-dependencies of crates outside this workspace**, which cargo does not resolve at all
//!    and which nothing here compiles.
//! 3. **A feature set a downstream consumer selects.** This is the graph *this* workspace builds.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;

/// **Every resolved dependency compiles on the MSRV the crate that reaches it advertises.**
///
/// Stated per workspace member rather than against one global number, because that is the shape of
/// the promise: `connectors-api` is `publish = false` and answers only to this repository's own
/// gate, while `connector-spec`, `connector-catalog`, `connector-pack` and `connector-secrets` are
/// on crates.io and answer to whoever `cargo add`ed them.
#[test]
fn no_resolved_dependency_declares_a_rust_version_above_the_crate_that_reaches_it() {
    let graph = Graph::read();

    // A fence over a graph carrying no MSRV data would pass forever while asserting nothing.
    graph.assert_it_can_see_an_msrv();

    let mut breaches = Vec::new();
    for member in graph.members() {
        let Some(declared) = graph.rust_version(member) else {
            continue;
        };
        for reached in graph.closure(member) {
            let Some(required) = graph.rust_version(&reached) else {
                continue;
            };
            if required > declared {
                breaches.push(format!(
                    "  {} declares rust-version {declared} and reaches {}, which requires {required}\n    {}",
                    graph.describe(member),
                    graph.describe(&reached),
                    graph
                        .path_to(member, &reached)
                        .unwrap_or_else(|| "<no path found>".to_owned()),
                ));
            }
        }
    }

    assert!(
        breaches.is_empty(),
        "{} workspace crate(s) resolve a dependency declaring a higher MSRV than they advertise:\n\
         {}\n\n\
         `rust-version` is inherited from `[workspace.package]` and four of these crates are \
         published, so a declared MSRV below what the crate compiles under is a promise that can \
         only be corrected in a later version. Either pin the dependency below the bump (a tilde \
         requirement, `~10.3.0`, is the shape that lets patches through and keeps the MSRV out), \
         or raise the workspace `rust-version` — which is a semver-relevant decision for the \
         published crates and belongs to the repository owner.",
        breaches.len(),
        breaches.join("\n"),
    );
}

/// **What CI actually does about the MSRV, asserted rather than assumed.**
///
/// This does not fail when the MSRV is untested — it fails when the workflow stops saying so. Every
/// Rust job in CI installs one pinned toolchain, and that toolchain is far above the declared
/// `rust-version`; nothing anywhere builds this workspace on 1.87. The resolver change in C-213
/// keeps a *newer* dependency out of the graph, and that is worth having, but it is not the same
/// claim as "the declared MSRV builds" and must not be read as one.
///
/// The assertion is deliberately weak — it checks the pin is above the MSRV, not that some
/// particular job exists — because `.github/workflows/` is coordinator territory and a story that
/// asserted a job layout would fight whoever edits it next. Adding a real MSRV job is
/// [C-213]'s recorded follow-up, not its content.
#[test]
fn ci_pins_a_toolchain_far_above_the_declared_msrv() {
    let root = workspace_root();
    let workflow = root.join(".github/workflows/ci.yml");
    let text = std::fs::read_to_string(&workflow)
        .unwrap_or_else(|error| panic!("read {}: {error}", workflow.display()));

    let pinned: Vec<Version> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("toolchain:"))
        .filter_map(|value| Version::parse(value.trim().trim_matches('"')))
        .collect();
    assert!(
        !pinned.is_empty(),
        "{} installs no pinned Rust toolchain, so this test no longer describes CI",
        workflow.display()
    );

    let declared = workspace_msrv(&root);
    for toolchain in &pinned {
        assert!(
            *toolchain > declared,
            "{} pins Rust {toolchain}, which is not above the declared MSRV {declared}.\n\
             If CI has gained a job that builds on the MSRV itself, that is the coverage this \
             repository was missing — update this test to say so.",
            workflow.display()
        );
    }
}

/// The declared workspace MSRV, read from `[workspace.package] rust-version`.
///
/// # Panics
///
/// If the manifest cannot be read, parsed, or does not declare one — the fence has no meaning
/// without it.
fn workspace_msrv(root: &Path) -> Version {
    let path = root.join("Cargo.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let document: toml::Value = text
        .parse()
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    let declared = document
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("rust-version"))
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| {
            panic!(
                "{} declares no `[workspace.package] rust-version`",
                path.display()
            )
        });
    Version::parse(declared)
        .unwrap_or_else(|| panic!("`rust-version = \"{declared}\"` is not a version"))
}

/// A Rust version, compared numerically. `1.9` is below `1.10`, which a string comparison gets
/// wrong, and `1.87` and `1.87.0` are the same version, which cargo permits both spellings of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version(u64, u64, u64);

impl Version {
    /// Parse `major.minor[.patch]`. Returns `None` for anything else; cargo forbids prerelease and
    /// wildcard spellings in `rust-version`, so there is no third case to handle.
    fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next().unwrap_or("0").parse().ok()?;
        let patch = parts.next().unwrap_or("0").parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self(major, minor, patch))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The feature-resolved dependency graph, keyed by cargo's opaque package ids so that two versions
/// of one crate stay distinct — which matters here, where the whole question is *which version*
/// was selected.
struct Graph {
    /// Package id to its display name, `name vX.Y.Z`.
    described: BTreeMap<String, String>,
    /// Package id to its declared `rust-version`, where it declares one.
    rust_versions: BTreeMap<String, Version>,
    /// Package id to the ids of the dependencies cargo resolved for it.
    edges: BTreeMap<String, Vec<String>>,
    /// The ids of this workspace's own members.
    members: BTreeSet<String>,
}

impl Graph {
    /// Read the graph cargo resolves for this workspace.
    ///
    /// `--locked` and `--offline` are both deliberate, and for the reasons
    /// [`pack_links_no_http_client.rs`](../pack_links_no_http_client.rs) gives: this must describe
    /// the committed lockfile, must not be able to change it, and a test in this repository
    /// reaching the network would be its own defect.
    ///
    /// Every dependency kind is admitted, dev included. A dev-dependency is compiled by this
    /// repository's own gate, so an MSRV the gate cannot honour is not a real MSRV either; and
    /// cargo does not resolve dev-dependencies of crates outside the workspace, so admitting them
    /// widens the graph by exactly the members' own test deps and nothing further.
    ///
    /// # Panics
    ///
    /// If cargo cannot be run, exits non-zero, or emits something that is not the metadata
    /// document — in a test each of those is the assertion failing, not a condition to recover
    /// from.
    fn read() -> Self {
        let root = workspace_root();
        let output = Command::new(env!("CARGO"))
            .args(["metadata", "--format-version", "1", "--locked", "--offline"])
            .current_dir(&root)
            .output()
            .unwrap_or_else(|error| panic!("run `cargo metadata` in {}: {error}", root.display()));
        assert!(
            output.status.success(),
            "`cargo metadata` failed in {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr)
        );

        let document: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("`cargo metadata` emits JSON");

        let mut described = BTreeMap::new();
        let mut rust_versions = BTreeMap::new();
        for package in document["packages"]
            .as_array()
            .expect("`packages` is an array")
        {
            let (Some(id), Some(name)) = (package["id"].as_str(), package["name"].as_str()) else {
                continue;
            };
            let version = package["version"].as_str().unwrap_or("?");
            described.insert(id.to_owned(), format!("{name} v{version}"));
            if let Some(declared) = package["rust_version"].as_str().and_then(Version::parse) {
                rust_versions.insert(id.to_owned(), declared);
            }
        }

        let mut edges = BTreeMap::new();
        for node in document["resolve"]["nodes"]
            .as_array()
            .expect("`resolve.nodes` is an array — `--no-deps` was not passed")
        {
            let Some(id) = node["id"].as_str() else {
                continue;
            };
            let dependencies = node["deps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|dep| dep["pkg"].as_str())
                .map(str::to_owned)
                .collect();
            edges.insert(id.to_owned(), dependencies);
        }

        let members = document["workspace_members"]
            .as_array()
            .expect("`workspace_members` is an array")
            .iter()
            .filter_map(|id| id.as_str())
            .map(str::to_owned)
            .collect();

        Self {
            described,
            rust_versions,
            edges,
            members,
        }
    }

    /// A graph stated directly, for asserting the walk itself.
    fn over(members: &[&str], nodes: &[(&str, &str, &[&str])]) -> Self {
        Self {
            described: nodes
                .iter()
                .map(|(id, _, _)| ((*id).to_owned(), (*id).to_owned()))
                .collect(),
            rust_versions: nodes
                .iter()
                .filter_map(|(id, msrv, _)| {
                    Version::parse(msrv).map(|version| ((*id).to_owned(), version))
                })
                .collect(),
            edges: nodes
                .iter()
                .map(|(id, _, deps)| {
                    (
                        (*id).to_owned(),
                        deps.iter().map(|d| (*d).to_owned()).collect(),
                    )
                })
                .collect(),
            members: members.iter().map(|m| (*m).to_owned()).collect(),
        }
    }

    /// This workspace's own members, in a stable order.
    fn members(&self) -> impl Iterator<Item = &String> {
        self.members.iter()
    }

    /// The `rust-version` a package declares, if it declares one. Most of the ecosystem does not,
    /// and a package that says nothing makes no promise this fence can check.
    fn rust_version(&self, id: &str) -> Option<Version> {
        self.rust_versions.get(id).copied()
    }

    /// `name vX.Y.Z` for a package id, which is otherwise an opaque URL-ish string.
    fn describe<'a>(&'a self, id: &'a str) -> &'a str {
        self.described.get(id).map_or(id, String::as_str)
    }

    /// Every package reachable from `root`, `root` excluded.
    fn closure(&self, root: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut pending = vec![root.to_owned()];
        while let Some(id) = pending.pop() {
            for dependency in self.edges.get(&id).into_iter().flatten() {
                if seen.insert(dependency.clone()) {
                    pending.push(dependency.clone());
                }
            }
        }
        seen
    }

    /// A rendered `a -> b -> c` chain from `root` to `target`, for the failure message. Breadth
    /// first, so the reported chain is a shortest one and names the edge worth changing.
    fn path_to(&self, root: &str, target: &str) -> Option<String> {
        let mut previous: BTreeMap<String, String> = BTreeMap::new();
        let mut queue = VecDeque::from([root.to_owned()]);
        let mut seen = BTreeSet::from([root.to_owned()]);
        while let Some(id) = queue.pop_front() {
            for dependency in self.edges.get(&id).into_iter().flatten() {
                if !seen.insert(dependency.clone()) {
                    continue;
                }
                previous.insert(dependency.clone(), id.clone());
                if dependency == target {
                    let mut chain = vec![self.describe(target).to_owned()];
                    let mut step = target;
                    while let Some(parent) = previous.get(step) {
                        chain.push(self.describe(parent).to_owned());
                        step = parent;
                    }
                    chain.reverse();
                    return Some(chain.join(" -> "));
                }
                queue.push_back(dependency.clone());
            }
        }
        None
    }

    /// The fence is only meaningful if the graph carries MSRV declarations at all. A metadata
    /// format change, or a `--no-deps` slipping into the invocation, would otherwise leave every
    /// comparison vacuously satisfied.
    fn assert_it_can_see_an_msrv(&self) {
        assert!(
            self.members().any(|id| self.rust_version(id).is_some()),
            "no workspace member declares a `rust-version`, so this fence compares against nothing"
        );
        let declaring_dependencies = self
            .rust_versions
            .keys()
            .filter(|id| !self.members.contains(*id))
            .count();
        assert!(
            declaring_dependencies > 0,
            "no dependency outside this workspace declares a `rust-version`, so this fence has \
             nothing to check — the graph is not the one it thinks it is reading"
        );
    }
}

/// The workspace root, from this crate's manifest directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate manifest lives two directories below the workspace root")
        .to_path_buf()
}

/// The fence above is only as good as the walk and the comparison under it, and both are the parts
/// that could pass while seeing nothing.
///
/// The transitive case is the one that actually happens — this repository's live instance is
/// `connectors-api -> flux-web -> flux-plugin -> zip`, three edges away and behind two crates
/// nobody here maintains — so it cannot be checked by editing a manifest without editing someone
/// else's.
#[test]
fn the_walk_finds_a_breach_that_is_not_direct() {
    let graph = Graph::over(
        &["member"],
        &[
            ("member", "1.87", &["helper", "quiet"][..]),
            ("helper", "1.85", &["late"][..]),
            ("late", "1.88", &[][..]),
            ("quiet", "", &[][..]),
        ],
    );

    let closure = graph.closure("member");
    assert!(
        closure.contains("late"),
        "a dependency two edges away must still be in the closure: {closure:?}"
    );
    assert_eq!(
        graph.path_to("member", "late").as_deref(),
        Some("member -> helper -> late")
    );
    // And the crate in between, whose own MSRV is *below* the member's, is not a breach.
    assert!(graph.rust_version("helper") < graph.rust_version("member"));
    // A package declaring nothing makes no promise, and must not be read as one.
    assert_eq!(graph.rust_version("quiet"), None);
}

/// A Rust version is compared numerically, not as a string. `1.9` versus `1.10` is the comparison a
/// lexical fence gets backwards, and it is not hypothetical — 1.100 is roughly two years out.
#[test]
fn versions_compare_numerically_and_tolerate_both_spellings() {
    let parse = |text: &str| Version::parse(text).expect("a version");

    assert!(
        parse("1.9") < parse("1.10"),
        "lexically, \"1.9\" > \"1.10\""
    );
    assert!(parse("1.87") < parse("1.88"));
    assert_eq!(parse("1.87"), parse("1.87.0"));
    assert!(parse("1.87.0") < parse("1.87.1"));
    assert_eq!(Version::parse("1.87.0-beta"), None);
    assert_eq!(Version::parse("1.2.3.4"), None);
}
