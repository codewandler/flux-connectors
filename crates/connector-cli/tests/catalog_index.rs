//! **The whole-catalogue artifacts are a function of a full run, and only of a full run** (C-104).
//!
//! Three documents in this repository describe the catalogue *as a whole* rather than one
//! provider: `crates/catalog/src/generated.rs` (the provider module index), `web/public/catalog.json`
//! (the site's catalogue), and the README's rendered SVGs. None of them can be written honestly from
//! a `--provider` or `--service` run, because such a run compiled a subset — writing one anyway
//! would drop every provider the run did not look at, and it would do so *successfully*. That is the
//! worst available failure: a green build, a committed index, and sixteen connectors silently gone.
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
use std::path::PathBuf;

mod common;

use common::Fixture;

/// `crates/catalog/src/generated.rs`, relative to a workspace root.
const CATALOG_INDEX: &str = "crates/catalog/src/generated.rs";

/// `web/public/catalog.json`, relative to a workspace root.
const SITE_CATALOG: &str = "web/public/catalog.json";

/// Every whole-catalogue artifact: the documents a scoped run must leave alone.
const WHOLE_CATALOGUE: &[&str] = &[
    CATALOG_INDEX,
    SITE_CATALOG,
    "assets/readme-snippet-light.svg",
    "assets/readme-snippet-dark.svg",
];

/// Run a command through the real parser and the real `run`, exactly as `main` does.
fn run(args: &[&str]) -> anyhow::Result<String> {
    let invocation = connector_cli::cli::parse(args.iter().map(|arg| arg.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("output is UTF-8"))
}

/// A fixture holding three providers — enough that a scoped run leaves a majority behind.
fn three_providers(label: &str) -> Fixture {
    let fixture = Fixture::with_provider(label, "acme");
    for provider in ["beacon", "cinder"] {
        fixture.write_provider(provider, &common::definition(provider));
        fixture.write_spec(provider, "v1", "{\"openapi\":\"3.0.3\"}\n");
    }
    fixture
}

/// The whole-catalogue files that exist in the tree, mapped to their bytes.
///
/// Absent files are recorded as absent rather than skipped, so "the run created one" is a
/// difference the comparison catches rather than one it tolerates.
fn whole_catalogue(fixture: &Fixture) -> BTreeMap<&'static str, Option<Vec<u8>>> {
    WHOLE_CATALOGUE
        .iter()
        .map(|relative| {
            let path: PathBuf = fixture.root().join(relative);
            (*relative, std::fs::read(&path).ok())
        })
        .collect()
}

/// **The property the whole story rests on.** A `--provider` run touches no whole-catalogue
/// document.
///
/// The tree is fully built first, so every global artifact holds all three providers. The scoped run
/// that follows compiles exactly one of them — and if any global document were a function of *that*
/// run, it would come back naming one provider and having silently dropped two. Byte-identical is
/// the only acceptable answer, and it is what makes two implementors' write sets disjoint.
#[test]
fn a_scoped_build_leaves_the_whole_catalogue_artifacts_byte_identical() {
    let fixture = three_providers("scoped-leaves-globals");
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("the full build succeeds");

    let before = whole_catalogue(&fixture);
    assert!(
        before[CATALOG_INDEX].is_some(),
        "a full build must write {CATALOG_INDEX}; a hand-maintained index is the file every \
         provider story collides on"
    );
    assert!(
        before[SITE_CATALOG].is_some(),
        "a full build must write {SITE_CATALOG}"
    );

    run(&["build", "--provider", "acme", "--root", &root]).expect("the scoped build succeeds");

    let after = whole_catalogue(&fixture);
    for relative in WHOLE_CATALOGUE {
        assert_eq!(
            before[relative].as_deref().map(String::from_utf8_lossy),
            after[relative].as_deref().map(String::from_utf8_lossy),
            "`build --provider acme` rewrote {relative} from one provider's data — the other two \
             are gone, and the build said nothing"
        );
    }
}

/// The same property for `--service`, which narrows the *contents* of every artifact and is
/// therefore no more able to write a complete index than `--provider` is.
#[test]
fn a_service_scoped_build_leaves_the_whole_catalogue_artifacts_byte_identical() {
    let fixture = three_providers("service-scoped-leaves-globals");
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("the full build succeeds");
    let before = whole_catalogue(&fixture);

    run(&["build", "--service", "default", "--root", &root]).expect("the scoped build succeeds");
    let after = whole_catalogue(&fixture);

    assert_eq!(
        before[CATALOG_INDEX]
            .as_deref()
            .map(String::from_utf8_lossy),
        after[CATALOG_INDEX].as_deref().map(String::from_utf8_lossy),
        "`build --service default` rewrote {CATALOG_INDEX}"
    );
    assert_eq!(
        before, after,
        "a service-scoped build rewrote a global index"
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
