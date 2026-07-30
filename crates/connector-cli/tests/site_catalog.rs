//! `web/public/catalog.json` is a **checked** artifact, and it is the one a website is written
//! against (C-42). It sits in VitePress's `public/` directory, which is served verbatim at the site
//! root, so the explorer reads it with no copy step (C-44).
//!
//! The site's whole reason for existing is that catalogue data must never be hand-maintained — that
//! is the action-proxy failure this repository exists to correct, re-enacted in JavaScript. So the
//! committed JSON is recomputed here from `providers/*.toml` and compared, exactly the way
//! `catalog_artifacts.rs` checks the Rust catalogue.
//!
//! Two of these assertions are not about staleness at all and are the reason the story exists:
//!
//! - **`status` is derived, not decorative.** An operation that does not work must say so, and say
//!   it from a rule applied to the IR rather than from a list someone maintains by hand.
//! - **No credential value ever reaches the document.** Env var *names* only. The check runs the
//!   real binary with a credential variable set to a sentinel and asserts the sentinel is nowhere in
//!   the output.
//!
//! The format itself is specified in `docs/designs/catalog-json.md`.

use std::path::{Path, PathBuf};

use connector_cli::pipeline;
use connector_cli::workspace::Workspace;
use serde_json::Value;

mod common;

use common::Fixture;

/// Every provider this repository ships: C-17's original three, then each connector added
/// since — `github` (C-52), `openai` (C-51), `slack` (C-53).
const SHIPPED: &[&str] = &["zendesk", "freshdesk", "babelforce", "github", "openai", "slack"];

/// The document's path, relative to the repository root. Chosen by C-42, moved into the site's own
/// `public/` tree by C-44, and named here so a change to it is a change to a test rather than a
/// silent break of whatever reads it.
const CATALOG_JSON: &str = "web/public/catalog.json";

/// The repository root, derived from this crate's manifest directory so the test is independent of
/// the working directory a runner happens to use.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// The committed document, parsed.
fn committed() -> Value {
    let path = repo_root().join(CATALOG_JSON);
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{CATALOG_JSON} is missing or unreadable ({error}) — the site has no catalogue to read. \
             Run `cargo run -p connector-cli -- build`"
        )
    });
    serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("{CATALOG_JSON} is not valid JSON: {error}"))
}

/// Every operation in the document, flattened across providers.
fn operations(document: &Value) -> Vec<&Value> {
    document["providers"]
        .as_array()
        .unwrap_or_else(|| panic!("{CATALOG_JSON} carries a `providers` array"))
        .iter()
        .flat_map(|provider| {
            provider["operations"]
                .as_array()
                .unwrap_or_else(|| panic!("every provider carries an `operations` array"))
        })
        .collect()
}

/// One operation by id.
fn operation<'a>(document: &'a Value, id: &str) -> &'a Value {
    operations(document)
        .into_iter()
        .find(|operation| operation["id"] == Value::String(id.to_string()))
        .unwrap_or_else(|| panic!("{CATALOG_JSON} carries the operation `{id}`"))
}

/// The `code` of every issue on an operation's status.
fn issue_codes(operation: &Value) -> Vec<String> {
    operation["status"]["issues"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "operation `{}` carries a `status.issues` array",
                operation["id"]
            )
        })
        .iter()
        .map(|issue| {
            issue["code"]
                .as_str()
                .expect("every issue carries a string `code`")
                .to_string()
        })
        .collect()
}

/// **The document is planned and committed like every other artifact.**
///
/// A build writes it, `diff` reports it stale, and the committed bytes are a fixed point of a
/// rebuild. Without this the site's data is generated in name only.
#[test]
fn the_build_writes_and_checks_site_catalog_json() {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("every shipped provider compiles");

    let planned: Vec<String> = plan
        .artifacts
        .iter()
        .map(|artifact| {
            workspace
                .display_path(&artifact.path)
                .display()
                .to_string()
                .replace('\\', "/")
        })
        .collect();

    assert!(
        planned.iter().any(|path| path == CATALOG_JSON),
        "a build plans no {CATALOG_JSON}; the site would have to hand-maintain the catalogue. \
         Planned artifacts:\n  {}",
        planned.join("\n  ")
    );

    let stale: Vec<String> = plan
        .changes()
        .map(|artifact| workspace.display_path(&artifact.path).display().to_string())
        .collect();
    assert!(
        stale.is_empty(),
        "a rebuild would change committed artifacts — run `cargo run -p connector-cli -- build`:\n  {}",
        stale.join("\n  ")
    );
}

/// **Every shipped operation is in the document**, with the fields the explorer is written against:
/// the metadata, the typed parameters with their JSON Schema, and the generated Flux verbatim.
#[test]
fn every_shipped_operation_carries_its_metadata_and_its_flux() {
    let document = committed();

    for provider in SHIPPED {
        let path = repo_root()
            .join("providers")
            .join(format!("{provider}.toml"));
        let source = std::fs::read_to_string(&path).expect("a shipped provider definition");
        let connector =
            connector_spec::provider::load(&format!("providers/{provider}.toml"), &source)
                .expect("a shipped provider loads")
                .connector;

        for declared in &connector.operations {
            let entry = operation(&document, &declared.id);
            assert_eq!(entry["provider"], Value::String(provider.to_string()));
            assert_eq!(entry["path"], Value::String(declared.path.clone()));
            assert!(
                entry["risk"].is_string() && entry["idempotency"].is_string(),
                "operation `{}` is missing its risk/idempotency",
                declared.id
            );

            let flux = entry["flux"].as_str().unwrap_or_else(|| {
                panic!(
                    "operation `{}` carries its Flux source verbatim",
                    declared.id
                )
            });
            let emitted = connector_flux::emit_operation(&connector, declared)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", declared.id));
            assert_eq!(
                flux, emitted,
                "the Flux in {CATALOG_JSON} for `{}` is not what the emitter produces",
                declared.id
            );

            // Typed parameters keep their JSON Schema — the whole point of carrying the IR's types
            // through rather than a stringly-typed shadow of them.
            let parameters = entry["parameters"].as_array().unwrap_or_else(|| {
                panic!("operation `{}` carries a `parameters` array", declared.id)
            });
            assert_eq!(parameters.len(), declared.params.iter().count());
            for parameter in parameters {
                assert!(
                    parameter["schema"].is_object(),
                    "parameter `{}` of `{}` lost its JSON Schema",
                    parameter["name"],
                    declared.id
                );
                assert!(parameter["in"].is_string());
            }
        }
    }
}

/// **`status` is derived from the IR, and it is the field that carries the honesty.**
///
/// Three facts the repository already states in prose, asserted here as machine-readable data:
///
/// - `zendesk-ticket-search` cannot percent-encode its query value (C-28/C-30), and the six other
///   zendesk operations are unaffected — the connector is honestly 6/7;
/// - every freshdesk operation ships with no credential at all (C-17), and zendesk's do not;
/// - `works` is true only when nothing is wrong, so a consumer can filter on one boolean.
#[test]
fn the_status_of_every_operation_is_derived_from_the_ir() {
    let document = committed();

    let search = operation(&document, "zendesk-ticket-search");
    assert!(
        issue_codes(search).contains(&"unencodable-query-value".to_string()),
        "`zendesk-ticket-search` does not declare the percent-encoding gap; it is published as \
         though it worked. Issues: {:?}",
        issue_codes(search)
    );

    let show = operation(&document, "zendesk-ticket-show");
    assert!(
        !issue_codes(show).contains(&"unencodable-query-value".to_string()),
        "`zendesk-ticket-show` takes only a numeric path id and is unaffected by the encoding gap, \
         but it is flagged for it — the refusal rule is too broad. Issues: {:?}",
        issue_codes(show)
    );

    for entry in operations(&document) {
        let codes = issue_codes(entry);
        let provider = entry["provider"].as_str().expect("a provider id");
        let has_no_credential = codes.iter().any(|code| code == "no-credential");

        if provider == "freshdesk" {
            assert!(
                has_no_credential,
                "freshdesk operation `{}` is published without saying it ships with no credential",
                entry["id"]
            );
        } else {
            assert!(
                !has_no_credential,
                "operation `{}` is flagged as having no credential, but `{provider}` declares one",
                entry["id"]
            );
        }

        assert_eq!(
            entry["status"]["works"],
            Value::Bool(codes.is_empty()),
            "operation `{}` reports `works` out of step with its issues ({codes:?})",
            entry["id"]
        );
    }
}

/// **No credential value, anywhere — env var names only.**
///
/// The check is a real build of a fixture connector, run as a subprocess with the credential's
/// environment variable set to a sentinel. If any future edit resolves a credential while
/// generating the document, the sentinel lands in the JSON and this fails. Asserting the *name* is
/// present as well is what keeps the test from passing because nothing about auth is emitted at all.
#[test]
fn no_credential_value_reaches_the_document() {
    const ENV_NAME: &str = "FLUX_CONNECTORS_C42_SENTINEL";
    const ENV_VALUE: &str = "sentinel-credential-value-must-never-be-emitted";

    let fixture = Fixture::new("c42-credential");
    fixture.write_provider(
        "acme",
        &format!(
            r#"id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"
description = "A hand-authored fixture connector."
default_auth = [{{ credentials = ["acme.token"] }}]

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["{ENV_NAME}"]
description = "The fixture credential."

[[operations]]
id = "acme-thing-get"
method = "GET"
path = "/v1/things/{{thing_id}}"
description = "Fetch one thing."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "thing_id"
required = true
schema = {{ type = "integer" }}
"#
        ),
    );

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_flux-connectors"))
        .args(["build", "--root"])
        .arg(fixture.root())
        .env(ENV_NAME, ENV_VALUE)
        .output()
        .expect("the flux-connectors binary runs");
    assert!(
        output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        fixture.exists(CATALOG_JSON),
        "a build wrote no {CATALOG_JSON}"
    );
    let document = fixture.read(CATALOG_JSON);

    assert!(
        !document.contains(ENV_VALUE),
        "{CATALOG_JSON} carries a resolved credential value — env var names only"
    );
    assert!(
        document.contains(ENV_NAME),
        "{CATALOG_JSON} does not name the credential's environment variable, so the test above \
         passes vacuously"
    );
}

/// **Rebuilding from unchanged inputs is byte-identical.** The document travels through
/// `pipeline::plan` like every other artifact, so an unchanged input writes nothing at all.
#[test]
fn rebuilding_the_document_writes_nothing() {
    let fixture = Fixture::with_provider("c42-determinism", "acme");
    let binary = env!("CARGO_BIN_EXE_flux-connectors");

    let first = std::process::Command::new(binary)
        .args(["build", "--root"])
        .arg(fixture.root())
        .output()
        .expect("the flux-connectors binary runs");
    assert!(first.status.success());
    assert!(
        fixture.exists(CATALOG_JSON),
        "a build wrote no {CATALOG_JSON}"
    );

    let before = fixture.snapshot();
    let second = std::process::Command::new(binary)
        .args(["build", "--root"])
        .arg(fixture.root())
        .output()
        .expect("the flux-connectors binary runs");
    assert!(second.status.success());

    assert_eq!(
        before,
        fixture.snapshot(),
        "a second build over unchanged inputs changed the tree"
    );
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("up to date"),
        "a second build did not report the tree up to date: {}",
        String::from_utf8_lossy(&second.stdout)
    );
}
