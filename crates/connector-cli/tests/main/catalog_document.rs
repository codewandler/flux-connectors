//! The canonical per-provider catalog document — C-536 (Decision 0022).
//!
//! A full build lowers every provider's IR to one deterministic, committed JSON document at
//! `catalog/<name>.catalog.json`, beside the `.flux` and `.connector.toml` it already emits. The
//! document is the reviewed artifact of Decision 0022: it carries the complete published surface,
//! including an explicit request template equivalent to what `connector-pack/src/request.rs`
//! derives from the emitted Flux body, and the four declared surfaces that reached no artifact
//! before it (`roles`, `quirks.pagination`, `quirks.rate_limit`, service-level data aside —
//! see `AGENTS.md` § Intentional gaps).
//!
//! Everything here reads the **plan**, not the committed tree, so a stale checkout fails the
//! fixed-point test below rather than silently measuring old bytes. The template-vs-Flux
//! differential lives with the evaluator that can answer it:
//! `crates/connector-pack/tests/document_differential.rs`.

use std::path::{Path, PathBuf};

use connector_cli::pipeline::{self, Change, Plan};
use connector_cli::workspace::Workspace;
use serde_json::Value;

/// Where the documents live, relative to the repository root — the design's diagram
/// (`docs/designs/catalog-artifact.md`) puts them beside `connectors/`.
const DOCUMENTS_DIR: &str = "catalog";

/// The one JSON Schema every document validates against, committed beside them.
const SCHEMA_PATH: &str = "catalog/connector-document.schema.json";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

fn full_plan() -> (Workspace, Plan) {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("every shipped provider compiles");
    (workspace, plan)
}

/// The planned bytes of one repository-relative artifact, or a panic naming what is missing.
fn planned_contents<'a>(workspace: &Workspace, plan: &'a Plan, relative: &str) -> &'a str {
    let path = workspace.root().join(relative);
    plan.artifacts
        .iter()
        .find(|artifact| artifact.path == path)
        .map(|artifact| artifact.contents.as_str())
        .unwrap_or_else(|| panic!("a full build must plan `{relative}` (C-536)"))
}

fn document(workspace: &Workspace, plan: &Plan, provider: &str) -> Value {
    let text = planned_contents(
        workspace,
        plan,
        &format!("{DOCUMENTS_DIR}/{provider}.catalog.json"),
    );
    serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("`{provider}`'s planned document is not JSON: {error}"))
}

/// The operation entry named `id`, out of a parsed document.
fn operation<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["operations"]
        .as_array()
        .expect("a document carries an operations array")
        .iter()
        .find(|operation| operation["id"] == id)
        .unwrap_or_else(|| panic!("the document must carry `{id}`"))
}

// ---------------------------------------------------------------------------------------------
// The artifact exists, per provider, and is a fixed point
// ---------------------------------------------------------------------------------------------

/// **The story's headline.** A full build plans one canonical document per provider, under the
/// `catalog/` root the design names.
#[test]
fn a_full_build_plans_one_canonical_document_per_provider() {
    let (workspace, plan) = full_plan();
    for provider in &plan.providers {
        planned_contents(
            &workspace,
            &plan,
            &format!("{DOCUMENTS_DIR}/{provider}.catalog.json"),
        );
    }
}

/// Unchanged inputs reproduce every document byte for byte: the committed documents are exactly
/// what a build would write, so a second build is a no-op. This is the same whole-catalogue
/// staleness shape `the_committed_lockfile_is_a_fixed_point_of_a_build` has, scoped to the new
/// artifact family — red in a provider story's worktree for the same reason, and resolved by the
/// coordinator's full build at integration.
#[test]
fn the_committed_documents_are_a_fixed_point_of_a_build() {
    let (workspace, plan) = full_plan();
    let documents_root = workspace.root().join(DOCUMENTS_DIR);
    let mut checked = 0usize;
    for artifact in &plan.artifacts {
        if !artifact.path.starts_with(&documents_root) {
            continue;
        }
        checked += 1;
        assert_eq!(
            artifact.change,
            Change::Unchanged,
            "`{}` is stale — run `cargo run -p connector-cli -- build`",
            artifact.path.display()
        );
    }
    assert!(
        checked > plan.providers.len(),
        "the plan must claim every document plus the schema under {DOCUMENTS_DIR}/ \
         (found {checked})"
    );
}

// ---------------------------------------------------------------------------------------------
// The request template
// ---------------------------------------------------------------------------------------------

/// The document carries an explicit request template — method, URL template, constant headers,
/// body encoding and template, endpoint slots — for the operation the README's own snippet shows.
#[test]
fn the_zendesk_document_carries_the_request_template() {
    let (workspace, plan) = full_plan();
    let document = document(&workspace, &plan, "zendesk");

    let update = operation(&document, "zendesk-ticket-update");
    let request = &update["request"];
    assert_eq!(request["method"], "PUT", "the template states the method");
    assert_eq!(
        request["url"], "{base}/api/v2/tickets/{ticket_id}",
        "the URL template interpolates the service base and the caller's path parameter"
    );
    assert_eq!(
        request["headers"]["content-type"], "application/json",
        "the media type is a constant header derived from the body encoding"
    );
    assert_eq!(request["body"]["encoding"], "json");
    assert_eq!(
        request["body"]["template"]["ticket"],
        serde_json::json!({ "$param": "ticket" }),
        "a caller body parameter is a `$param` splice at its wire path"
    );
    assert_eq!(
        update["endpoint"]["subdomain"],
        serde_json::json!(["host"]),
        "the endpoint slot names where the configured value lands (C-214's vocabulary)"
    );

    // The service half the template's `{base}` resolves against, template preserved.
    let services = document["services"]
        .as_array()
        .expect("a document carries its services");
    assert!(
        services
            .iter()
            .any(|service| service["base_url"] == "https://{subdomain}.zendesk.com"),
        "the zendesk service publishes its templated base URL"
    );

    // A query-placed parameter is structured data, never URL text (C-30).
    let incremental = operation(&document, "zendesk-incremental-ticket-list");
    assert_eq!(
        incremental["request"]["query"],
        serde_json::json!([{ "name": "start_time", "value": { "$param": "start_time" } }]),
    );
    assert!(
        incremental["request"]["body"].is_null(),
        "an operation that sends no body carries no body template"
    );
}

// ---------------------------------------------------------------------------------------------
// The four surfaces reaching their first artifact
// ---------------------------------------------------------------------------------------------

/// `roles`, `quirks.pagination` and the token-endpoint quirks were declared and reached nothing
/// (`AGENTS.md` § Intentional gaps). The document is their first artifact. `quirks.rate_limit`
/// has no declaration in the catalogue today (`providers/hubspot.toml` records a deliberate
/// non-declaration), so its coverage is the schema's — asserted in the schema test below.
#[test]
fn the_declared_surfaces_with_no_artifact_reach_the_document() {
    let (workspace, plan) = full_plan();

    // The one declared service role: anthropic's `models` service.
    let anthropic = document(&workspace, &plan, "anthropic");
    assert!(
        anthropic["services"]
            .as_array()
            .expect("services")
            .iter()
            .any(|service| service["roles"] == serde_json::json!(["llm_catalogue"])),
        "anthropic's declared service role must reach its document"
    );

    // Pagination quirks: declared on operations across the catalogue; at least one must surface.
    let paginated = plan
        .providers
        .iter()
        .filter(|provider| {
            document(&workspace, &plan, provider)["operations"]
                .as_array()
                .expect("operations")
                .iter()
                .any(|operation| !operation["quirks"]["pagination"].is_null())
        })
        .count();
    assert!(
        paginated > 0,
        "declared `quirks.pagination` must reach at least one document"
    );

    // babelforce's measured token-endpoint departures (C-440).
    let babelforce = document(&workspace, &plan, "babelforce");
    assert!(
        babelforce["auth"]
            .as_array()
            .expect("auth")
            .iter()
            .any(|method| {
                method["token_endpoint_quirks"]
                    .as_array()
                    .is_some_and(|quirks| !quirks.is_empty())
            }),
        "babelforce's token-endpoint quirks must reach its document"
    );

    // The error envelope, structurally rather than as prose appended to a description.
    let enveloped = plan
        .providers
        .iter()
        .filter(|provider| {
            document(&workspace, &plan, provider)["operations"]
                .as_array()
                .expect("operations")
                .iter()
                .any(|operation| {
                    operation["quirks"]["error_envelope"]["message_pointer"].is_string()
                })
        })
        .count();
    assert!(
        enveloped > 0,
        "a declared error envelope must reach the document as data"
    );
}

// ---------------------------------------------------------------------------------------------
// The schema: published, validated, and with no field for a registration value
// ---------------------------------------------------------------------------------------------

/// The document schema is a planned, committed artifact, and every planned document validates
/// against it — the `core_catalog.rs` pattern, applied to the connector documents.
#[test]
fn every_planned_document_validates_against_the_planned_schema() {
    let (workspace, plan) = full_plan();
    let schema: Value = serde_json::from_str(planned_contents(&workspace, &plan, SCHEMA_PATH))
        .expect("the planned schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("the planned schema compiles");

    let mut validated = 0usize;
    for provider in &plan.providers {
        let document = document(&workspace, &plan, provider);
        if let Err(error) = validator.validate(&document) {
            panic!("`{provider}`'s document does not validate: {error}");
        }
        validated += 1;
    }
    assert_eq!(validated, plan.providers.len());
}

/// Every property name any object in `schema` admits, wherever it sits.
fn property_names(schema: &Value, into: &mut Vec<String>) {
    match schema {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get("properties") {
                into.extend(properties.keys().cloned());
            }
            for value in object.values() {
                property_names(value, into);
            }
        }
        Value::Array(values) => {
            for value in values {
                property_names(value, into);
            }
        }
        _ => {}
    }
}

/// **No field for an OAuth2 registration value exists** — not empty, absent, and unrepresentable:
/// no object anywhere in the schema admits a `client_id` or `client_secret` property, and the
/// `oauth2` object is closed, so a future document cannot carry a value for a consumer to
/// mistakenly trust. The registration *requirement* is still published — through the existing
/// `binds = "oauth.*"` configuration grammar, which is the one legitimate spelling.
#[test]
fn the_schema_has_no_field_for_a_registration_value() {
    let (workspace, plan) = full_plan();
    let schema_text = planned_contents(&workspace, &plan, SCHEMA_PATH);

    let schema: Value = serde_json::from_str(schema_text).expect("the schema is JSON");
    let oauth2 = &schema["$defs"]["oauth2"];
    assert_eq!(
        oauth2["additionalProperties"],
        Value::Bool(false),
        "the oauth2 object must be closed, or absence of the field proves nothing"
    );
    let mut admitted = Vec::new();
    property_names(&schema, &mut admitted);
    assert!(
        !admitted.is_empty(),
        "the schema declares object properties"
    );
    for forbidden in ["client_id", "client_secret"] {
        assert!(
            !admitted.iter().any(|name| name == forbidden),
            "no object in the document schema may admit a `{forbidden}` property"
        );
    }
    // The rate-limit quirk is representable even though nothing declares one yet — the schema is
    // where that surface's coverage lives until a provider states one.
    assert!(
        schema_text.contains("rate_limit"),
        "the schema must carry the rate_limit quirk surface"
    );

    // The sharp case: both shipped OAuth2 connectors carried `client_id: ""` in the generated
    // catalogue. The vestigial value must not survive into their documents — while the
    // *requirement* does, as gitlab's operator-level `binds = "oauth.client_id"` field.
    for provider in ["gitlab", "babelforce"] {
        let document = document(&workspace, &plan, provider);
        let mut oauth2_declared = 0usize;
        for method in document["auth"].as_array().expect("auth").iter() {
            let Some(oauth2) = method.get("oauth2").filter(|spec| !spec.is_null()) else {
                continue;
            };
            oauth2_declared += 1;
            assert!(
                oauth2["token_path"].is_string(),
                "`{provider}` declares OAuth2, so its document carries the complete spec"
            );
            for forbidden in ["client_id", "client_secret"] {
                assert!(
                    oauth2.get(forbidden).is_none(),
                    "`{provider}`'s oauth2 must carry no `{forbidden}`"
                );
            }
        }
        assert!(
            oauth2_declared > 0,
            "`{provider}` declares OAuth2, so its document must carry the spec"
        );
    }

    // The requirement half: gitlab publishes the registration as operator-level configuration.
    let gitlab = document(&workspace, &plan, "gitlab");
    assert!(
        gitlab["config"]
            .as_array()
            .expect("config")
            .iter()
            .any(|field| field["binds"] == "oauth.client_id" && field["level"] == "operator"),
        "the registration requirement is published through the `binds` grammar, never as a value"
    );
}

// ---------------------------------------------------------------------------------------------
// Scoped runs
// ---------------------------------------------------------------------------------------------

/// A `--provider` run writes its own document and nobody else's — the disjoint-write-set property
/// that lets provider stories run in parallel, extended to the new family.
#[test]
fn a_provider_scoped_run_plans_its_own_document_only() {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, Some("zendesk")).expect("zendesk compiles");
    let documents_root = workspace.root().join(DOCUMENTS_DIR);

    let planned: Vec<String> = plan
        .artifacts
        .iter()
        .filter(|artifact| artifact.path.starts_with(&documents_root))
        .map(|artifact| {
            artifact
                .path
                .file_name()
                .expect("a planned document has a file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        planned.contains(&"zendesk.catalog.json".to_string()),
        "a scoped run plans its own provider's document; planned: {planned:?}"
    );
    assert!(
        !planned.iter().any(|name| name == "github.catalog.json"),
        "a scoped run must not plan another provider's document; planned: {planned:?}"
    );
}
