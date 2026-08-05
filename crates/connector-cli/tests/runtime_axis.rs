//! **How a connector executes reaches every artifact a host reads** — C-405.
//!
//! `catalog::Provider` carried no runtime, so a host could not read *how* a connector executes and
//! had to derive it. Every shipped connector is HTTP, so the derivation is right today and stays
//! right until the first one that is not — at which point it is silently wrong for exactly the case
//! a multi-tenant host's refusal exists to catch. flux's own rule is that a locally-executing
//! runtime (`process`, `container`, `socket`, `plugin`) cannot be safely multi-tenant in one
//! process, and that refusal is only mechanical if the runtime is a **declared fact** a host reads.
//!
//! So this asserts the whole path, not the IR: a provider declaring `runtime = "process"` must be
//! readable as `process` in the `.connector.toml` manifest, in `web/public/catalog.json`, and in the
//! generated Rust catalogue. `AGENTS.md`'s *Intentional gaps* lists six surfaces that the IR models
//! and no artifact carries; a test that stopped at the IR would file a seventh.
//!
//! Everything here reads the **planned** artifacts rather than the committed tree. Two of the three
//! are whole-catalogue artifacts the coordinator regenerates at integration (`AGENTS.md`,
//! "Whole-catalogue artifacts are coordinator-owned"), and planning needs nothing written to the
//! repository.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use connector_cli::pipeline;
use connector_cli::workspace::Workspace;
use connector_spec::Runtime;
use serde_json::Value;

mod common;

use common::Fixture;

/// The three artifacts under test, by their tail path.
const MANIFEST: &str = "connectors/acme.connector.toml";
const CATALOG_JSON: &str = "web/public/catalog.json";
const RUST_CATALOGUE: &str = "crates/catalog/src/generated/acme.rs";

/// A one-operation connector, optionally declaring a runtime.
///
/// `None` is the shape every shipped provider has: no `runtime` key at all, which is what makes
/// `http` the default worth having.
fn connector(runtime: Option<&str>) -> String {
    let declared = runtime
        .map(|runtime| format!("runtime = {runtime:?}\n"))
        .unwrap_or_default();
    format!(
        r#"id = "acme"
vendor = "Acme Inc."
base_url = "https://api.acme.example"
description = "A hand-authored fixture connector."
{declared}
[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List the things."
risk = "low"
idempotency = "idempotent"
"#
    )
}

/// Every planned artifact of a build over a fixture holding exactly this connector.
struct Planned {
    manifest: String,
    catalogue: Value,
    rust: String,
}

fn plan(label: &str, runtime: Option<&str>) -> Planned {
    let fixture = Fixture::new(label);
    fixture.write_provider("acme", &connector(runtime));

    let workspace = Workspace::new(fixture.root().to_path_buf());
    let plan = pipeline::plan(&workspace, None).expect("the fixture connector compiles");

    let contents = |tail: &str| -> String {
        plan.artifacts
            .iter()
            .find(|artifact| artifact.path.ends_with(tail))
            .unwrap_or_else(|| panic!("a full build plans {tail}"))
            .contents
            .clone()
    };

    Planned {
        manifest: contents(MANIFEST),
        catalogue: serde_json::from_str(&contents(CATALOG_JSON))
            .expect("the planned catalogue is valid JSON"),
        rust: contents(RUST_CATALOGUE),
    }
}

/// **The headline claim.** A connector that executes as a spawned process says so in all three
/// artifacts, so a host refusing a locally-executing runtime reads a fact instead of guessing one.
#[test]
fn a_non_http_runtime_reaches_the_manifest_the_catalogue_and_the_rust_tables() {
    let planned = plan("runtime-process", Some("process"));

    assert!(
        planned.manifest.contains("runtime = \"process\""),
        "the manifest must name the runtime — it is the file a host reads to decide whether it may \
         run this connector at all. Got:\n{}",
        planned.manifest
    );
    assert_eq!(
        planned.catalogue["providers"][0]["runtime"],
        Value::String("process".to_owned()),
        "`{CATALOG_JSON}` must publish the declared runtime; a consumer that has to derive it is \
         the derivation this story removes"
    );
    assert!(
        planned.rust.contains("runtime: crate::Runtime::Process,"),
        "the generated Rust catalogue must carry the runtime as a typed field, so a host reading \
         `catalog::Provider` never has to infer it. Got:\n{}",
        planned.rust
    );
}

/// **And the default is published, not merely assumed.** A provider declaring nothing is `http`, and
/// says `http` — an absent field would leave a consumer inferring exactly what it infers today.
#[test]
fn a_connector_declaring_no_runtime_publishes_http() {
    let planned = plan("runtime-default", None);

    assert!(
        planned.manifest.contains("runtime = \"http\""),
        "an undeclared runtime must still be stated as `http`; publishing nothing would put the \
         derivation back. Got:\n{}",
        planned.manifest
    );
    assert_eq!(
        planned.catalogue["providers"][0]["runtime"],
        Value::String("http".to_owned())
    );
    assert!(
        planned.rust.contains("runtime: crate::Runtime::Http,"),
        "got:\n{}",
        planned.rust
    );
}

/// The repository root, from this crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("`crates/connector-cli` sits two levels below the repository root")
        .to_path_buf()
}

/// **The two `Runtime` enums are one vocabulary.**
///
/// `connector_spec::Runtime` is what a provider file declares; `catalog::Runtime` is what a consumer
/// of the published Rust catalogue matches on. They are separate types because `connector-catalog`
/// is deliberately dependency-free (`AGENTS.md`, ownership boundaries) — it cannot import the
/// loader's enum — and two hand-written copies of one closed set is precisely the seam a mirrored
/// vocabulary stops being closed at.
///
/// Half of the agreement is already structural: `crate::catalog::runtime` matches exhaustively on
/// `connector_spec::Runtime` and names a `catalog::Runtime` variant per arm, so a variant added on
/// either side fails to compile. That half is blind in one direction — a `catalog::Runtime` variant
/// the loader has no counterpart for compiles fine and is simply unreachable — so this reads the
/// catalog crate's source and holds the two sets equal.
///
/// A source-text check rather than a linked one because this crate must not depend on `catalog`:
/// `crates/connector-cli/tests/dependency_fence.rs` sorts every workspace member into compiler, host
/// library or host, and an edge from the compiler's CLI to the catalogue crate is not one of them.
/// Reading a committed file in a test is the same shape `msrv_fence.rs` and
/// `pack_links_no_http_client.rs` already use.
#[test]
fn the_catalogue_crate_mirrors_the_loaders_runtime_vocabulary() {
    let source = std::fs::read_to_string(repo_root().join("crates/catalog/src/lib.rs"))
        .expect("the catalog crate's source is readable");

    let body = source
        .split_once("pub enum Runtime {")
        .expect("`catalog::Runtime` is declared")
        .1
        .split_once("\n}")
        .expect("the enum body is closed")
        .0;

    // A variant is a line that is neither a doc comment nor an attribute, with its comma stripped.
    let published: BTreeSet<String> = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("///") && !line.starts_with('#'))
        .map(|line| line.trim_end_matches(',').to_owned())
        .collect();

    // The loader's own, spelled the way a Rust variant is: `llm_catalogue`-style words upper-camelled.
    let accepted: BTreeSet<String> = Runtime::ALL
        .iter()
        .map(|runtime| {
            let word = runtime.word();
            let mut camel = String::with_capacity(word.len());
            for segment in word.split('_') {
                let mut characters = segment.chars();
                if let Some(first) = characters.next() {
                    camel.extend(first.to_uppercase());
                    camel.push_str(characters.as_str());
                }
            }
            camel
        })
        .collect();

    assert_eq!(
        published, accepted,
        "`catalog::Runtime` and `connector_spec::Runtime` name different sets. The vocabulary is \
         flux's runtime axis (`docs/designs/ecosystem.md`), mirrored here twice because the \
         catalogue crate takes no dependencies — and a mirrored closed set that nothing verifies \
         stops being closed at exactly this seam"
    );
}
