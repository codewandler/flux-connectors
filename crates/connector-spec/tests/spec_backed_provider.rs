//! The spec front-end end to end: `[spec]` + `[[patch.operations]]` in, `Connector` out — C-4.
//!
//! `openapi_ingest.rs` covers the document half. This covers the **join**: which of the ingested
//! operations a provider file publishes, what it may correct about each, and — more importantly —
//! what the loader refuses rather than deciding on the author's behalf.
//!
//! The document under test is the trimmed Zendesk excerpt committed under `specs/`, read off disk
//! for the reason `shipped_providers.rs` reads `providers/` off disk: a copy embedded here would be
//! the thing under test drifting away from the thing that ships.

use std::path::{Path, PathBuf};

use connector_spec::{provider, Connector, Idempotency, Risk, SpecDocument};

/// The repository-relative path every fixture below pins, spelled as `[spec] path` spells it.
const PINNED: &str = "specs/zendesk/2024-06-01-excerpt.json";

fn document() -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PINNED);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// A spec cache holding exactly the pinned document.
fn cache() -> Vec<SpecDocument<'static>> {
    // Leaked so the fixtures can hand out a `SpecDocument<'static>` without threading a lifetime
    // through every helper; a test process is the one place that costs nothing.
    let document: &'static str = Box::leak(document().into_boxed_str());
    vec![SpecDocument {
        path: PINNED,
        document,
    }]
}

/// The connector `definition` compiles to against the excerpt.
fn load(definition: &str) -> Connector {
    provider::load_with_spec("providers/zendesk.toml", definition, &cache())
        .unwrap_or_else(|error| panic!("providers/zendesk.toml does not load: {error}"))
        .connector
}

/// The problems `definition` produces, rendered as the author would read them.
fn refuse(definition: &str) -> String {
    let error = provider::load_with_spec("providers/zendesk.toml", definition, &cache())
        .err()
        .unwrap_or_else(|| panic!("this definition was expected not to load:\n{definition}"));
    error.to_string()
}

/// The `[spec]` pointer every fixture below carries.
const POINTER: &str = "\
id = \"zendesk\"
vendor = \"Zendesk\"
base_url = \"https://acme.zendesk.com\"

[spec]
path = \"specs/zendesk/2024-06-01-excerpt.json\"
";

fn with(patch: &str) -> String {
    format!("{POINTER}{patch}")
}

// ---------------------------------------------------------------------------------------------
// Selection is opt-in
// ---------------------------------------------------------------------------------------------

/// **The property this whole story rests on.** The excerpt makes four operations available, and a
/// file that names none of them publishes none of them.
///
/// Stated as a test rather than left to inference because the failure is silent in the dangerous
/// direction: an ingest that helpfully published everything it found would turn a 398-operation
/// vendor document into 398 LLM tools, which is a denial of service against a model's context and
/// is exactly why `Patch` has no `hide`. Widening *selection* is C-6's and C-411's; widening it by
/// accident is nobody's.
#[test]
fn a_spec_backed_provider_with_no_patch_publishes_nothing() {
    let connector = load(POINTER);
    assert!(
        connector.operations.is_empty(),
        "ingest selected on the author's behalf: {:?}",
        connector
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(connector.base_url, "https://acme.zendesk.com");
}

/// The whole document is kept available to patch, including the operations nothing selected. That
/// is what "ingest makes everything available; it selects nothing" means concretely.
#[test]
fn everything_the_document_declares_stays_available_to_patch() {
    let loaded = provider::load_with_spec("providers/zendesk.toml", POINTER, &cache())
        .expect("a pointer with no patch is a valid provider file");
    let ingested = loaded
        .ingested
        .as_ref()
        .expect("a document was supplied, so it was ingested");
    let mut available = ingested.operation_ids();
    available.sort_unstable();
    assert_eq!(
        available,
        vec!["createTicket", "deleteTicket", "listTickets", "showTicket"]
    );
    assert!(
        loaded.connector.operations.is_empty(),
        "available is not the same as published"
    );
    assert!(
        !loaded.diagnostics().is_empty(),
        "the excerpt carries deliberate defects and they must be reported"
    );
}

/// A selected operation arrives complete: the vendor's method, path, description, parameters and
/// response shape, plus the risk and idempotency the author had to state.
#[test]
fn a_selected_operation_carries_the_documents_request_and_the_authors_judgement() {
    let connector = load(&with(
        "
[[patch.operations]]
select = \"showTicket\"
rename = \"zendesk-ticket-show\"
risk = \"low\"
idempotency = \"idempotent\"
",
    ));

    let operation = connector
        .operation("zendesk-ticket-show")
        .expect("the renamed operation");
    assert_eq!(operation.path, "/api/v2/tickets/{ticket_id}");
    assert_eq!(operation.description, "Show one ticket by id.");
    assert_eq!(operation.risk, Risk::Low);
    assert_eq!(operation.idempotency, Idempotency::Idempotent);
    assert_eq!(operation.params.path[0].name, "ticket_id");
    assert!(
        operation.response_schema.is_some(),
        "the vendor's 2xx schema must travel into the op contract"
    );
}

/// A file may select from the document **and** write operations inline. Both roles the provider file
/// plays produce the same `Connector`, which is what "two front-ends, one IR" means in practice.
#[test]
fn inline_operations_and_selected_ones_land_in_one_connector() {
    let connector = load(&with(
        "
[[operations]]
id = \"zendesk-hand-written\"
method = \"GET\"
path = \"/api/v2/users/me\"
risk = \"low\"
idempotency = \"idempotent\"

[[patch.operations]]
select = \"listTickets\"
rename = \"zendesk-ticket-list\"
risk = \"low\"
idempotency = \"idempotent\"
",
    ));

    let ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(ids, vec!["zendesk-hand-written", "zendesk-ticket-list"]);
}

// ---------------------------------------------------------------------------------------------
// What the loader refuses rather than deciding
// ---------------------------------------------------------------------------------------------

/// A `select` that matches nothing is loud. A silent no-op is how a patch set rots underneath a
/// vendor's rename: the operation disappears from the connector and the build stays green.
#[test]
fn a_select_that_names_no_operation_is_refused_and_suggests_the_near_misses() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"showticket\"
rename = \"zendesk-ticket-show\"
risk = \"low\"
idempotency = \"idempotent\"
",
    ));
    assert!(rendered.contains("showticket"), "{rendered}");
    assert!(
        rendered.contains("specs/zendesk/2024-06-01-excerpt.json"),
        "the refusal must name the document searched: {rendered}"
    );
    assert!(
        rendered.contains("showTicket"),
        "a casing slip deserves the spelling that would have worked: {rendered}"
    );
}

/// An operation the ingest **skipped** is not selectable, and the refusal is the same one an absent
/// operation gets — which is the point: a diagnostic plus a green build would leave an author
/// hunting for an operation that was reported missing three lines earlier.
#[test]
fn an_operation_the_ingest_skipped_cannot_be_selected() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"fixtureMultipartUpload\"
rename = \"zendesk-upload\"
risk = \"medium\"
idempotency = \"non_idempotent\"
",
    ));
    assert!(rendered.contains("fixtureMultipartUpload"), "{rendered}");
}

/// **Silence about risk refuses.** No OpenAPI document publishes `risk` or `idempotency`, and
/// neither has a `Default` in this IR precisely so that a safety decision cannot be made by an
/// omission. Deriving them from the HTTP method is the failure mode this repository has legislated
/// against twice; ingest does not get to make it a third time.
#[test]
fn a_selection_that_states_no_risk_or_idempotency_is_refused() {
    for (patch, expected) in [
        (
            "select = \"deleteTicket\"\nrename = \"zendesk-ticket-delete\"\nidempotency = \"non_idempotent\"",
            "`risk`",
        ),
        (
            "select = \"deleteTicket\"\nrename = \"zendesk-ticket-delete\"\nrisk = \"destructive\"",
            "`idempotency`",
        ),
        (
            "select = \"deleteTicket\"\nrename = \"zendesk-ticket-delete\"",
            "`risk` and no `idempotency`",
        ),
    ] {
        let rendered = refuse(&with(&format!("\n[[patch.operations]]\n{patch}\n")));
        assert!(
            rendered.contains(expected),
            "the refusal must name what is missing ({expected}): {rendered}"
        );
    }
}

/// **An op id is a public contract**, so `operationId` is never promoted into one. Users and models
/// call an operation by name and a vendor's `operationId` is a volatile field; deriving one from the
/// other silently is what `docs/designs/connector-pipeline.md` refuses under "Op naming is a public
/// contract". C-412 replaces the per-operation `rename` with a rule declared once — it does not
/// remove the requirement to decide.
#[test]
fn a_selection_that_states_no_rename_is_refused_rather_than_taking_the_operation_id() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"listTickets\"
risk = \"low\"
idempotency = \"idempotent\"
",
    ));
    assert!(rendered.contains("listTickets"), "{rendered}");
    assert!(rendered.contains("rename"), "{rendered}");
}

/// A parameter correction that matches nothing is the same rot as a `select` that does, one level
/// down: the vendor renamed a field and the correction that used to fix its type silently stopped
/// applying.
#[test]
fn a_parameter_correction_that_matches_nothing_is_refused() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"listTickets\"
rename = \"zendesk-ticket-list\"
risk = \"low\"
idempotency = \"idempotent\"

[[patch.operations.params]]
name = \"page_size\"
position = \"query\"
required = true
",
    ));
    assert!(rendered.contains("page_size"), "{rendered}");
}

/// The corrections that do match are applied — a wrong type, a false `required`, a missing
/// description. This is the pressure valve the whole `[spec]` front-end rests on: a vendor that
/// types a date as a bare string must be correctable without hand-writing the operation.
#[test]
fn a_parameter_correction_that_matches_is_applied() {
    let connector = load(&with(
        "
[[patch.operations]]
select = \"listTickets\"
rename = \"zendesk-ticket-list\"
description = \"List tickets, newest first.\"
risk = \"low\"
idempotency = \"idempotent\"

[[patch.operations.params]]
name = \"page\"
position = \"query\"
required = true
description = \"Which page to fetch, one-based.\"
schema = { type = \"integer\", minimum = 1, maximum = 100 }
",
    ));

    let operation = connector
        .operation("zendesk-ticket-list")
        .expect("selected");
    assert_eq!(operation.description, "List tickets, newest first.");
    let page = operation
        .params
        .query
        .iter()
        .find(|param| param.name == "page")
        .expect("the corrected parameter");
    assert!(page.required, "the correction must win over the vendor");
    assert_eq!(page.description, "Which page to fetch, one-based.");
    assert_eq!(
        page.schema,
        serde_json::json!({ "type": "integer", "minimum": 1, "maximum": 100 })
    );
}

/// A selected operation is validated by **the same pass** a hand-authored one is, not by a second,
/// weaker one. A rename that is not a legal op id has to fail here rather than at emission.
#[test]
fn a_selected_operation_is_held_to_every_rule_an_inline_one_is() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"showTicket\"
rename = \"zendesk-ticket-show\"
risk = \"low\"
idempotency = \"idempotent\"
auth = [{ credentials = [\"zendesk.api_token\"] }]
",
    ));
    assert!(
        rendered.contains("zendesk.api_token"),
        "a selected operation requiring an undeclared credential must be refused: {rendered}"
    );
}

// ---------------------------------------------------------------------------------------------
// The document is read because the file asked for one
// ---------------------------------------------------------------------------------------------

/// A file with no `[spec]` block ignores the document entirely. `specs/<provider>/` holding a file
/// is not a declaration — `[spec] path` is — which is what keeps all forty-five hand-authored
/// connectors compiling exactly as they did, spec cache or no spec cache.
#[test]
fn a_hand_authored_file_ignores_a_document_supplied_beside_it() {
    let definition = "\
id = \"zendesk\"
base_url = \"https://acme.zendesk.com\"

[[operations]]
id = \"zendesk-hand-written\"
method = \"GET\"
path = \"/api/v2/users/me\"
risk = \"low\"
idempotency = \"idempotent\"
";
    let loaded = provider::load_with_spec("providers/zendesk.toml", definition, &cache())
        .expect("a hand-authored file loads whatever sits in the cache beside it");
    assert!(loaded.is_hand_authored());
    assert!(loaded.ingested.is_none(), "nothing asked for an ingest");
    assert!(loaded.diagnostics().is_empty());
    assert_eq!(loaded.connector.operations.len(), 1);
}

/// A document that is not an OpenAPI document at all fails the provider, naming the path the file
/// points at — the file is what an author can open and fix.
#[test]
fn a_document_that_cannot_be_ingested_fails_the_provider_naming_the_spec_path() {
    let error = provider::load_with_spec(
        "providers/zendesk.toml",
        POINTER,
        &[SpecDocument {
            path: PINNED,
            document: "{\"swagger\": \"2.0\"}",
        }],
    )
    .expect_err("a Swagger 2.0 file is not an OpenAPI 3.x document");
    let rendered = error.to_string();
    assert!(rendered.contains(PINNED), "{rendered}");
    assert!(rendered.contains("openapi"), "{rendered}");
}

// ---------------------------------------------------------------------------------------------
// The pin decides which document is compiled
// ---------------------------------------------------------------------------------------------

/// **`[spec] path` selects the document, and the cache ordinarily holds more than one.**
///
/// `specs/<provider>/` is a cache of *versions of one document*, so a pin beside a newer file is the
/// ordinary state, not an exotic one. Resolving by anything other than the pin — file order, recency
/// — compiles an operation out of a document the provider file never named, and does it
/// successfully: the operation exists in both, so nothing downstream can tell.
#[test]
fn the_pinned_document_is_compiled_even_when_a_later_one_sits_beside_it() {
    // Same `operationId`, different request — modelled on babelforce's real `getUser` collision.
    const NEWER: &str = r#"{
      "openapi": "3.0.3",
      "servers": [{"url": "https://acme.zendesk.com"}],
      "paths": {"/api/v2/wrong": {"get": {"operationId": "showTicket", "summary": "Not this one."}}}
    }"#;

    let pinned = document();
    let loaded = provider::load_with_spec(
        "providers/zendesk.toml",
        &with(
            "
[[patch.operations]]
select = \"showTicket\"
rename = \"zendesk-ticket-show\"
risk = \"low\"
idempotency = \"idempotent\"
",
        ),
        &[
            SpecDocument {
                path: PINNED,
                document: &pinned,
            },
            SpecDocument {
                path: "specs/zendesk/2025-01-01.json",
                document: NEWER,
            },
        ],
    )
    .expect("the pinned document is in the cache");

    assert_eq!(
        loaded
            .connector
            .operation("zendesk-ticket-show")
            .expect("selected")
            .path,
        "/api/v2/tickets/{ticket_id}",
        "the pin was read as a label and a different document was compiled"
    );
}

/// A pin that resolves to nothing is refused, and the refusal lists what the cache holds — a message
/// naming only the pin sends an author looking for a typo in the wrong file.
#[test]
fn a_pin_that_resolves_to_nothing_is_refused_and_names_the_cache() {
    let rendered = provider::load_with_spec(
        "providers/zendesk.toml",
        POINTER,
        &[SpecDocument {
            path: "specs/zendesk/2025-01-01.json",
            document: "{\"openapi\":\"3.0.3\"}",
        }],
    )
    .expect_err("the pinned document is not in the cache")
    .to_string();
    assert!(rendered.contains(PINNED), "{rendered}");
    assert!(
        rendered.contains("specs/zendesk/2025-01-01.json"),
        "{rendered}"
    );

    let empty = provider::load_with_spec("providers/zendesk.toml", POINTER, &[])
        .expect_err("an empty cache cannot satisfy a pin")
        .to_string();
    assert!(empty.contains(PINNED), "{empty}");
    assert!(empty.contains("no document"), "{empty}");
}

/// **The declared `sha256` is checked against the bytes ingested, not copied past them.**
///
/// It travels into `Provenance::spec_sha256` and from there into `connectors.lock`. Unchecked, the
/// lockfile would record a hash for bytes nothing ever hashed — provenance as a claim the file makes
/// about itself. Checking *upstream* drift is a different question and is C-14's.
#[test]
fn a_declared_spec_hash_that_disagrees_with_the_document_is_refused() {
    let pinned = document();
    let definition = format!("{POINTER}sha256 = \"{}\"\n", "0".repeat(64));
    let rendered = provider::load_with_spec(
        "providers/zendesk.toml",
        &definition,
        &[SpecDocument {
            path: PINNED,
            document: &pinned,
        }],
    )
    .expect_err("the declared hash is not the document's")
    .to_string();
    assert!(rendered.contains("sha256"), "{rendered}");
    assert!(rendered.contains(&"0".repeat(64)), "{rendered}");

    // And the honest declaration loads, so the check is a check and not a blanket refusal.
    let honest = format!(
        "{POINTER}sha256 = \"{}\"\n",
        connector_spec::sha256_hex(pinned.as_bytes())
    );
    let loaded = provider::load_with_spec(
        "providers/zendesk.toml",
        &honest,
        &[SpecDocument {
            path: PINNED,
            document: &pinned,
        }],
    )
    .expect("a declaration that matches the bytes");
    assert_eq!(
        loaded.connector.provenance.spec_sha256.as_deref(),
        Some(connector_spec::sha256_hex(pinned.as_bytes()).as_str())
    );
}

/// Loading the same bytes twice produces the same connector — `connectors.lock` hashes the IR, so a
/// leaked iteration order anywhere in ingest or selection would surface as phantom drift.
#[test]
fn loading_a_spec_backed_provider_is_deterministic() {
    let definition = with(
        "
[[patch.operations]]
select = \"createTicket\"
rename = \"zendesk-ticket-create\"
risk = \"medium\"
idempotency = \"non_idempotent\"

[[patch.operations]]
select = \"showTicket\"
rename = \"zendesk-ticket-show\"
risk = \"low\"
idempotency = \"idempotent\"
",
    );
    assert_eq!(load(&definition), load(&definition));
    assert_eq!(
        load(&definition).canonical_json().expect("the IR encodes"),
        load(&definition).canonical_json().expect("the IR encodes"),
    );
}
