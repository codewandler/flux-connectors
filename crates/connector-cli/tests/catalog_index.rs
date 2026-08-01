//! **The whole-catalogue artifacts are a function of a full run, and only of a full run** (C-104).
//!
//! Five artifacts in this repository describe the catalogue *as a whole* rather than one provider:
//! `crates/catalog/src/generated.rs` (the provider module index), `web/public/catalog.json` (the
//! site's catalogue), `web/public/v1/**` (the published Flux core catalogue), the README's
//! rendered SVGs, and `connectors.lock` (the drift record — C-189, one row per provider). None can
//! be written honestly from a `--provider` or `--service` run, because such a
//! run compiled a subset — writing one anyway would drop every provider the run did not look at, and
//! it would do so *successfully*. That is the worst available failure: a green build, a committed
//! index, and sixteen connectors silently gone.
//!
//! **The scoping assertion compares the whole tree, not a list of paths.** An enumerated list is the
//! obvious way to write this and the wrong one: it silently stops covering a member the moment the
//! pipeline grows one, which is exactly the drift this file exists to prevent. Comparing full
//! snapshots means a sixth whole-catalogue artifact is covered the day it is added, by a test nobody
//! had to remember to update — and it is what covered `connectors.lock` for free the day C-189
//! landed it. The named constants below survive only for the *existence* checks, where naming the
//! file is the point.
//!
//! `catalog.json` has always been emitted on a full run only; `docs/designs/catalog-json.md` records
//! the rule. `generated.rs` reached the same conclusion from the other direction and paid for it by
//! being hand-maintained — which made it the one file every provider story had to append to, and so
//! the reason two provider stories could never run at once. C-104 makes it generated under the same
//! full-run rule, which is what lets provider work fan out: two implementors' write sets are now
//! disjoint.
//!
//! So this file asserts the *scoping* property rather than staleness. The staleness half lives in
//! `catalog_artifacts.rs`, whose whole-tree fixed-point assertion covers the index for free once it
//! is planned, and in `crates/catalog/tests/embedded_operations.rs`, which holds the committed index
//! against `providers/`.
//!
//! Everything here runs against a **fixture** tree with more than one provider, because the property
//! is about what a partial run does to the other providers' data — and one provider cannot exhibit
//! it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod common;

use common::Fixture;

/// `crates/catalog/src/generated.rs`, relative to a workspace root.
const CATALOG_INDEX: &str = "crates/catalog/src/generated.rs";

/// `web/public/catalog.json`, relative to a workspace root.
const SITE_CATALOG: &str = "web/public/catalog.json";

/// The README snippet's source, whose two rendered SVGs are a whole-catalogue artifact.
const SNIPPET: &str = "assets/readme-snippet.flux";

/// The published core catalogue's directory: `web/public/v1/**`.
const SITE_CORE_DIR: &str = "web/public/v1";

/// Run a command through the real parser and the real `run`, exactly as `main` does.
fn run(args: &[&str]) -> anyhow::Result<String> {
    let invocation = connector_cli::cli::parse(args.iter().map(|arg| arg.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("output is UTF-8"))
}

/// The repository root, for the committed inputs the fixture borrows.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// A fixture holding three providers, **and the inputs that make every whole-catalogue artifact
/// actually get produced**.
///
/// The core catalogue and the README snippet are copied from the repository rather than invented.
/// The core bundle is schema-checked with cross-record invariants, so a hand-rolled minimal one
/// would be a second thing to keep valid; and the point is coverage of `web/public/v1/**`, which a
/// fixture without that input never emits at all. A test that silently produced none of those files
/// would assert the scoping property over an empty set and pass forever.
fn three_providers(label: &str) -> Fixture {
    let fixture = Fixture::with_provider(label, "acme");
    for provider in ["beacon", "cinder"] {
        fixture.write_provider(provider, &common::definition(provider));
        fixture.write_spec(provider, "v1", "{\"openapi\":\"3.0.3\"}\n");
    }

    let core = std::fs::read_to_string(repo_root().join("specs/flux/core-v1.json"))
        .expect("the committed core catalogue is readable");
    fixture.write("specs/flux/core-v1.json", &core);

    let snippet = std::fs::read_to_string(repo_root().join(SNIPPET))
        .expect("the committed README snippet is readable");
    fixture.write(SNIPPET, &snippet);

    fixture
}

/// Every file under `relative`, as paths relative to the fixture root.
fn files_under(fixture: &Fixture, relative: &str) -> Vec<PathBuf> {
    fixture
        .snapshot()
        .into_keys()
        .filter(|path| path.starts_with(relative))
        .collect()
}

/// The paths whose bytes differ between two whole-tree snapshots, including files added or removed.
///
/// Returned as sorted strings rather than as a byte diff so a failure names the artifact a reader
/// has to go and look at, instead of printing a generated document at them.
fn changed(before: &BTreeMap<PathBuf, Vec<u8>>, after: &BTreeMap<PathBuf, Vec<u8>>) -> Vec<String> {
    let mut paths: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| path.display().to_string())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Assert that a full build produced every member of the class, so the comparison that follows is
/// over a non-empty set rather than over nothing.
///
/// This is the guard that keeps the whole-tree comparison honest: snapshot equality holds trivially
/// if the build wrote no whole-catalogue artifact at all.
fn assert_every_member_was_produced(fixture: &Fixture) {
    assert!(
        fixture.exists(CATALOG_INDEX),
        "a full build must write {CATALOG_INDEX}; a hand-maintained index is the file every \
         provider story collides on"
    );
    assert!(
        fixture.exists(SITE_CATALOG),
        "a full build must write {SITE_CATALOG}"
    );
    assert!(
        fixture.exists("assets/readme-snippet-light.svg")
            && fixture.exists("assets/readme-snippet-dark.svg"),
        "a full build must render both README images"
    );
    assert!(
        !files_under(fixture, SITE_CORE_DIR).is_empty(),
        "a full build must publish {SITE_CORE_DIR}/**; without it this test asserts the scoping \
         property over a member that was never produced"
    );
    assert!(
        fixture.exists(connector_spec::LOCKFILE_NAME),
        "a full build must write {}; a lockfile nothing produces is the defect C-189 closed",
        connector_spec::LOCKFILE_NAME
    );
}

/// **The property the whole story rests on.** A `--provider` run touches no whole-catalogue
/// artifact.
///
/// The tree is fully built first, so every global artifact holds all three providers. The scoped run
/// that follows compiles exactly one of them — and if any global document were a function of *that*
/// run, it would come back naming one provider and having silently dropped two.
///
/// The comparison is over the **entire tree**, so it covers all four members of the class and any
/// fifth that is ever added. Per-provider artifacts are covered too, and they are equal for a
/// different reason: the full build already made them current, so the scoped rebuild is a no-op. The
/// combined statement is the strong one — *a scoped rebuild of an up-to-date tree writes nothing at
/// all* — and it is what makes two implementors' write sets disjoint.
#[test]
fn a_scoped_build_leaves_the_whole_catalogue_artifacts_byte_identical() {
    let fixture = three_providers("scoped-leaves-globals");
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("the full build succeeds");
    assert_every_member_was_produced(&fixture);
    let before = fixture.snapshot();

    run(&["build", "--provider", "acme", "--root", &root]).expect("the scoped build succeeds");
    let after = fixture.snapshot();

    assert_eq!(
        changed(&before, &after),
        Vec::<String>::new(),
        "`build --provider acme` rewrote a file it had no complete input for — a whole-catalogue \
         artifact rendered from one provider's data has silently dropped the other two, and the \
         build reported success"
    );
}

/// The same property for `--service`, which narrows the *contents* of every artifact and is
/// therefore no more able to write a complete index than `--provider` is.
#[test]
fn a_service_scoped_build_leaves_the_whole_catalogue_artifacts_byte_identical() {
    let fixture = three_providers("service-scoped-leaves-globals");
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("the full build succeeds");
    assert_every_member_was_produced(&fixture);
    let before = fixture.snapshot();

    run(&["build", "--service", "default", "--root", &root]).expect("the scoped build succeeds");
    let after = fixture.snapshot();

    assert_eq!(
        changed(&before, &after),
        Vec::<String>::new(),
        "`build --service default` rewrote a whole-catalogue artifact"
    );
}

/// **A full build writes the index, and it names every provider it compiled.**
///
/// The other half of the same rule: leaving the index alone on a scoped run is only sound because a
/// full run does produce it. An index nothing generates is one a human maintains, which is where
/// this story started.
#[test]
fn a_full_build_writes_an_index_naming_every_provider() {
    let fixture = three_providers("full-build-writes-index");
    run(&["build", "--root", fixture.root().to_str().unwrap()]).expect("the build succeeds");

    let index = fixture.read(CATALOG_INDEX);
    for provider in ["acme", "beacon", "cinder"] {
        assert!(
            index.contains(&format!("pub(crate) mod {provider};")),
            "{CATALOG_INDEX} does not declare `{provider}`'s module:\n{index}"
        );
        assert!(
            index.contains(&format!("&{provider}::PROVIDER,")),
            "{CATALOG_INDEX} does not list `{provider}` in PROVIDERS:\n{index}"
        );
    }
}

/// The index is a checked artifact like any other: `diff` reports it stale, and a second build
/// writes nothing.
#[test]
fn the_index_is_a_checked_artifact() {
    let fixture = three_providers("index-is-checked");
    let root = fixture.root().to_str().unwrap().to_string();
    run(&["build", "--root", &root]).expect("the build succeeds");

    let second = run(&["build", "--root", &root]).expect("the rebuild succeeds");
    assert!(
        second.contains("nothing written"),
        "an unchanged index must not be rewritten: {second}"
    );

    fixture.write(CATALOG_INDEX, "// hand-edited\n");
    let output = run(&["diff", "--root", &root]).expect("diff succeeds");
    assert!(
        output.contains(CATALOG_INDEX),
        "`diff` must report a hand-edited index as stale: {output}"
    );
}

/// **A provider whose name is not a Rust identifier is refused, loudly.**
///
/// `providers/` admits `-` in a file stem — it becomes the artifact file name and the op-id prefix,
/// where a hyphen is fine. A Rust `mod` declaration is the one place it is not, and while the index
/// was hand-written a human would have hit the compile error immediately. Generated, it would ship a
/// `crates/catalog` that does not build, from an input that looks perfectly ordinary. AGENTS.md's
/// rule applies: refuse at emission rather than emit plausible-but-broken output.
#[test]
fn a_provider_name_that_is_not_a_rust_identifier_is_refused() {
    let fixture = Fixture::with_provider("index-refuses-non-identifier", "acme");
    fixture.write_provider("google-ads", &common::definition("google-ads"));
    fixture.write_spec("google-ads", "v1", "{\"openapi\":\"3.0.3\"}\n");

    let error = run(&["build", "--root", fixture.root().to_str().unwrap()])
        .expect_err("a provider that cannot be a module name must not build");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("google-ads"),
        "the refusal must name the provider: {rendered}"
    );
}
