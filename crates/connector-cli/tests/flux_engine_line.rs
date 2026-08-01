//! **The flux engine line, written down once and checked** (C-403).
//!
//! Seven `codewandler-flux-*` requirements sit in `[workspace.dependencies]`, and until C-403 the
//! line they were on existed only as six copies of a version string plus several paragraphs of
//! prose reasoning about them. That is the shape a bump takes three days to do: every copy is a
//! place to be inconsistent, and nothing fails when one of them is.
//!
//! So the line is recorded here — [`ENGINE_LINE`] and [`SPEC_LINE`], two constants — and the
//! manifest is required to state it. **The next bump is a value change**: edit the constant, run
//! this test, and it names every requirement still on the old line.
//!
//! # Two lines, because flux ships two kinds of crate
//!
//! The engine crates (`flux-lang`, `flux-core`, `flux-runtime`, `flux-web`, `flux-system`,
//! `flux-credentials`) move together on one `0.x` line; a `0.x` requirement is
//! `>=0.N.0, <0.N+1.0` to cargo, so two of them on different lines is two incompatible copies of
//! flux in one lock. `flux-spec` is the **frozen wire vocabulary** a guest plugin compiles against
//! and moves on its own `1.x` line — it is not a slower engine crate, it is a different promise,
//! and conflating the two is what [`the_spec_line_is_recorded_separately_from_the_engine`] exists
//! to stop.
//!
//! # Which graph, and why two of them
//!
//! [`every_flux_requirement_states_the_recorded_line`] reads the **manifest**, because that is
//! where a pin is authored and where a bump is done. [`the_lock_carries_one_engine_line`] reads
//! the **lock**, because a manifest that states one line proves nothing about what resolved: a
//! transitive dependency requiring `^0.41` alongside this workspace's `0.45` puts both in the tree,
//! and the manifest is silent about it. That split is the exact hazard the `flux-web` and
//! `flux-credentials` comments in the root manifest record having checked for by hand.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// **The flux engine line this repository is built against.** Bumping flux starts here.
///
/// A `0.x` line, so this is the whole compatibility unit: `"0.45"` admits every `0.45.z` and
/// nothing else.
const ENGINE_LINE: &str = "0.47";

/// **The `flux-spec` line**, which is not the engine's — see this module's documentation.
const SPEC_LINE: &str = "1.3";

/// The one `codewandler-flux-*` package that is on [`SPEC_LINE`] rather than [`ENGINE_LINE`].
const SPEC_PACKAGE: &str = "codewandler-flux-spec";

/// The prefix that makes a package one of flux's.
const FLUX_PREFIX: &str = "codewandler-flux-";

/// **Every `codewandler-flux-*` requirement in the workspace manifest states the recorded line.**
///
/// Derived from the manifest rather than listed here, so a flux crate added later is covered by
/// arriving rather than by someone remembering to extend a list.
#[test]
fn every_flux_requirement_states_the_recorded_line() {
    let requirements = flux_requirements();

    // A fence over an empty set passes forever while asserting nothing. Seven is what C-403 moved;
    // the assertion is that there are some, not that there are exactly seven, because a new flux
    // crate should not have to edit this test to be checked by it.
    assert!(
        !requirements.is_empty(),
        "no `{FLUX_PREFIX}*` requirement found in [workspace.dependencies] — this test is reading \
         the wrong table and would pass whatever the pins said"
    );

    let mut wrong = Vec::new();
    for (package, stated) in &requirements {
        let expected = expected_line(package);
        if stated != expected {
            wrong.push(format!(
                "  {package} = \"{stated}\", expected \"{expected}\""
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "these `[workspace.dependencies]` requirements are not on the recorded flux line \
         (engine {ENGINE_LINE}, {SPEC_PACKAGE} {SPEC_LINE}):\n{}\n\
         The line is recorded in `ENGINE_LINE`/`SPEC_LINE` at the top of this file. Change the \
         constant and the manifest together — that pairing is the whole point of recording it.",
        wrong.join("\n")
    );
}

/// **The lock carries one engine line, not two.**
///
/// The failure this catches is the one the root manifest's `flux-credentials` comment records
/// measuring by hand: a flux crate whose own requirements sit a line behind drags a second copy of
/// `flux-core` into the tree, and both copies build. Two `flux_runtime::Tool` traits in one process
/// is precisely the linking failure C-403 exists to end, so it is asserted rather than re-measured
/// each time.
///
/// Only the `0.x` crates are the engine. flux also publishes stable-vocabulary crates
/// (`flux-spec`, `flux-policy`, `flux-secret`, `flux-evidence`, …) that are on `1.x` lines of their
/// own and are not versioned with the engine at all.
#[test]
fn the_lock_carries_one_engine_line() {
    let locked = locked_flux_versions();

    assert!(
        !locked.is_empty(),
        "no `{FLUX_PREFIX}*` package found in `Cargo.lock` — this test is reading the wrong file"
    );

    let mut off_line = Vec::new();
    for (package, versions) in &locked {
        for version in versions {
            if version.starts_with("0.") && !on_line(version, ENGINE_LINE) {
                off_line.push(format!("  {package} {version}"));
            }
        }
    }

    assert!(
        off_line.is_empty(),
        "`Cargo.lock` resolves flux engine crates off the recorded line {ENGINE_LINE}:\n{}\n\
         A `0.x` requirement is semver-incompatible across minor versions, so each of these is a \
         second, separate copy of flux in the same build — two `flux_runtime::Tool` traits, and a \
         host that can link one of them.",
        off_line.join("\n")
    );
}

/// **The spec line is recorded separately, and is genuinely a different line.**
///
/// Written as an assertion rather than a comment because the tempting simplification — one constant
/// for "the flux version" — is wrong in a way that builds: `flux-spec = "0.45"` resolves to nothing
/// and `flux-spec = "1.3"` written as the engine line would put every engine crate on a version
/// that does not exist.
#[test]
fn the_spec_line_is_recorded_separately_from_the_engine() {
    assert_ne!(
        SPEC_LINE, ENGINE_LINE,
        "`{SPEC_PACKAGE}` is the frozen wire vocabulary and moves on its own line; recording it as \
         the engine line collapses two different promises into one number"
    );

    let locked = locked_flux_versions();
    let spec = locked
        .get(SPEC_PACKAGE)
        .unwrap_or_else(|| panic!("`Cargo.lock` carries no `{SPEC_PACKAGE}`"));

    for version in spec {
        assert!(
            on_line(version, SPEC_LINE),
            "`Cargo.lock` resolves {SPEC_PACKAGE} {version}, which is not on the recorded line \
             {SPEC_LINE}"
        );
    }
}

/// The line `package` is required to state.
fn expected_line(package: &str) -> &'static str {
    if package == SPEC_PACKAGE {
        SPEC_LINE
    } else {
        ENGINE_LINE
    }
}

/// Whether `version` (`0.45.0`) falls inside `line` (`0.45`).
///
/// String comparison over the leading components rather than a semver parse: a `0.x` line is two
/// components and a `1.x` line is one, so "the line" is the recorded prefix and nothing more.
fn on_line(version: &str, line: &str) -> bool {
    version == line || version.starts_with(&format!("{line}."))
}

/// Every `codewandler-flux-*` requirement in the root `[workspace.dependencies]`, keyed by package.
///
/// The key in that table is an alias (`flux-lang`), so the package name comes from the entry's own
/// `package` field — the same aliasing `publish_closure.rs` has to resolve.
fn flux_requirements() -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("the workspace manifest is readable");
    let document: toml::Value = text.parse().expect("the workspace manifest parses");
    let table = document
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .expect("the workspace manifest declares [workspace.dependencies]");

    let mut requirements = BTreeMap::new();
    for (alias, entry) in table {
        let package = entry
            .get("package")
            .and_then(toml::Value::as_str)
            .unwrap_or(alias.as_str());
        if !package.starts_with(FLUX_PREFIX) {
            continue;
        }
        let version = entry
            .get("version")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("`{package}` states no version in [workspace.dependencies]"));
        requirements.insert(package.to_owned(), version.to_owned());
    }
    requirements
}

/// Every `codewandler-flux-*` package in `Cargo.lock`, and each version of it the lock carries.
///
/// A `Vec` per package rather than one version, because the whole question is whether a package
/// appears more than once.
fn locked_flux_versions() -> BTreeMap<String, Vec<String>> {
    let text = std::fs::read_to_string(workspace_root().join("Cargo.lock"))
        .expect("Cargo.lock is readable");
    let document: toml::Value = text.parse().expect("Cargo.lock parses");
    let packages = document
        .get("package")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock declares [[package]] entries");

    let mut locked: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in packages {
        let Some(name) = entry.get("name").and_then(toml::Value::as_str) else {
            continue;
        };
        if !name.starts_with(FLUX_PREFIX) {
            continue;
        }
        let version = entry
            .get("version")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("`{name}` has no version in Cargo.lock"));
        locked
            .entry(name.to_owned())
            .or_default()
            .push(version.to_owned());
    }
    locked
}

/// The workspace root, two directories above this crate's manifest.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate manifest lives two directories below the workspace root")
        .to_path_buf()
}
