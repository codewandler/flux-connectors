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
    let ingested = &loaded
        .ingested
        .first()
        .expect("a document was supplied, so it was ingested")
        .ingested;
    let mut available = ingested.operation_ids();
    available.sort_unstable();
    assert_eq!(
        available,
        vec![
            "createTicket",
            "deleteTicket",
            "listTickets",
            "showOrganization",
            "showTicket"
        ]
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
direction = \"read\"
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
direction = \"read\"
path = \"/api/v2/users/me\"
risk = \"low\"
idempotency = \"idempotent\"

[[patch.operations]]
select = \"listTickets\"
direction = \"read\"
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

/// **Plain [`provider::load`] refuses a spec-backed file rather than answering with a skeleton**
/// (C-421).
///
/// A spec-backed connector's operations are a function of the file's bytes *and* of the documents it
/// pins. `load` is handed only the first, so it is being asked a question it does not have the input
/// to answer — and until C-421 it answered anyway, with the id, the base URL, the credentials, the
/// provenance and **no operations at all**, and returned `Ok`. That is the "plausible but incorrect"
/// outcome `AGENTS.md` refuses: every catalogue-wide test in this repository reads `providers/` and
/// would have gone on passing over a connector it believed it had checked.
///
/// The refusal is narrow by construction. It fires on a pinned `[spec]` and nothing else, so the
/// fifty-three hand-authored providers load exactly as they did, and the same bytes handed to
/// [`provider::load_with_spec`] with the cache compile — which is what makes this a missing input
/// rather than a policy about spec-backed files.
#[test]
fn plain_load_refuses_a_spec_backed_file_rather_than_returning_a_skeleton() {
    let definition = with(
        "
[[patch.operations]]
select = \"showTicket\"
direction = \"read\"
rename = \"zendesk-ticket-show\"
risk = \"low\"
idempotency = \"idempotent\"
",
    );

    let rendered = provider::load("providers/zendesk.toml", &definition)
        .expect_err(
            "a spec-backed file loaded with no cache must refuse, not answer with a skeleton",
        )
        .to_string();
    assert!(
        rendered.contains(PINNED),
        "the refusal names the document it could not read: {rendered}"
    );
    assert!(
        rendered.contains("load_with_spec"),
        "the refusal names the entry point that takes the cache: {rendered}"
    );

    // The same bytes, with the cache the file asks for, compile. The refusal above is about an
    // input this entry point cannot accept, not about the file being wrong.
    let connector = load(&definition);
    assert_eq!(
        connector
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        vec!["zendesk-ticket-show"]
    );

    // And a file that pins nothing still loads through the pure entry point, unchanged.
    let hand_authored = "\
id = \"zendesk\"
vendor = \"Zendesk\"
base_url = \"https://acme.zendesk.com\"

[[operations]]
id = \"zendesk-hand-written\"
method = \"GET\"
direction = \"read\"
path = \"/api/v2/users/me\"
risk = \"low\"
idempotency = \"idempotent\"
";
    let pure = provider::load("providers/zendesk.toml", hand_authored)
        .expect("a hand-authored file needs no cache");
    assert_eq!(pure.connector.operations.len(), 1);
}

/// A `select` that matches nothing is loud. A silent no-op is how a patch set rots underneath a
/// vendor's rename: the operation disappears from the connector and the build stays green.
#[test]
fn a_select_that_names_no_operation_is_refused_and_suggests_the_near_misses() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"showticket\"
direction = \"read\"
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
direction = \"write\"
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
            "select = \"deleteTicket\"\ndirection = \"write\"\nrename = \"zendesk-ticket-delete\"\nidempotency = \"non_idempotent\"",
            "`risk`",
        ),
        (
            "select = \"deleteTicket\"\ndirection = \"write\"\nrename = \"zendesk-ticket-delete\"\nrisk = \"destructive\"",
            "`idempotency`",
        ),
        (
            "select = \"deleteTicket\"\ndirection = \"write\"\nrename = \"zendesk-ticket-delete\"",
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
direction = \"read\"
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
direction = \"read\"
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
direction = \"read\"
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
direction = \"read\"
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
direction = \"read\"
path = \"/api/v2/users/me\"
risk = \"low\"
idempotency = \"idempotent\"
";
    let loaded = provider::load_with_spec("providers/zendesk.toml", definition, &cache())
        .expect("a hand-authored file loads whatever sits in the cache beside it");
    assert!(loaded.is_hand_authored());
    assert!(loaded.ingested.is_empty(), "nothing asked for an ingest");
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
direction = \"read\"
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
direction = \"write\"
rename = \"zendesk-ticket-create\"
risk = \"medium\"
idempotency = \"non_idempotent\"

[[patch.operations]]
select = \"showTicket\"
direction = \"read\"
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

// ---------------------------------------------------------------------------------------------
// One connector, many documents — a document per service (C-410)
// ---------------------------------------------------------------------------------------------

/// The `manager` half of the two-document fixture: root `oauth2`, no operation override.
///
/// Modelled on babelforce's `manager-2026-07-10`, which declares root `oauth2` and **zero**
/// operation-level overrides across all 356 of its operations.
const MANAGER: &str = r#"{
  "openapi": "3.0.3",
  "info": { "title": "Acme Manager", "version": "0.0.0-dev" },
  "servers": [{ "url": "https://services.acme.example" }],
  "security": [{ "oauth2": [] }],
  "paths": {
    "/api/v2/users/{user_id}": {
      "get": {
        "operationId": "getUser",
        "summary": "Fetch one managed user.",
        "parameters": [
          { "name": "user_id", "in": "path", "required": true, "schema": { "type": "string" } }
        ]
      }
    }
  }
}
"#;

/// The `user` half: a different request under the **same** `operationId`, with per-operation
/// security instead of a root declaration.
///
/// This is babelforce's real collision reduced to one operation each: `getUser` is declared by
/// `manager-2026-07-10` *and* by `user-2026-06-25`, and they are not the same call.
const USER: &str = r#"{
  "openapi": "3.0.3",
  "info": { "title": "Acme User", "version": "0.0.0-dev" },
  "servers": [{ "url": "https://services.acme.example" }],
  "paths": {
    "/api/v3/me": {
      "get": {
        "operationId": "getUser",
        "summary": "Fetch the calling user.",
        "security": [{ "bearerAuth": [] }, { "oauth2": [] }]
      }
    }
  }
}
"#;

const MANAGER_PATH: &str = "specs/acme/manager-2026-07-10.json";
const USER_PATH: &str = "specs/acme/user-2026-06-25.json";

/// A cache holding both documents, in the order the directory would yield them.
fn two_documents() -> Vec<SpecDocument<'static>> {
    vec![
        SpecDocument {
            path: MANAGER_PATH,
            document: MANAGER,
        },
        SpecDocument {
            path: USER_PATH,
            document: USER,
        },
    ]
}

/// A connector declaring both documents, each joining its own service.
///
/// `default_auth` is stated at the connector level so the auth test below has something for an
/// unstated operation to inherit.
const TWO_SPECS: &str = r#"id = "acme"
vendor = "Acme"
base_url = "https://services.acme.example"

[[auth]]
name = "acme.oauth_token"
scheme = { header = { name = "Authorization", prefix = "Bearer " } }
env = ["ACME_OAUTH_TOKEN"]

[[default_auth]]
credentials = ["acme.oauth_token"]

[[services]]
name = "manager"
description = "The management API."

[[services]]
name = "user"
description = "The user API."

[[spec]]
path = "specs/acme/manager-2026-07-10.json"
service = "manager"

[[spec]]
path = "specs/acme/user-2026-06-25.json"
service = "user"
"#;

fn load_many(patch: &str) -> Connector {
    provider::load_with_spec(
        "providers/acme.toml",
        &format!("{TWO_SPECS}{patch}"),
        &two_documents(),
    )
    .unwrap_or_else(|error| panic!("providers/acme.toml does not load: {error}"))
    .connector
}

fn refuse_many(patch: &str) -> String {
    provider::load_with_spec(
        "providers/acme.toml",
        &format!("{TWO_SPECS}{patch}"),
        &two_documents(),
    )
    .err()
    .unwrap_or_else(|| panic!("this definition was expected not to load:\n{TWO_SPECS}{patch}"))
    .to_string()
}

/// The patch set selecting `getUser` out of **both** documents.
const BOTH_GET_USER: &str = r#"
[[patch.operations]]
service = "manager"
select = "getUser"
direction = "read"
rename = "acme-manager-user-get"
risk = "low"
idempotency = "idempotent"

[[patch.operations]]
service = "user"
select = "getUser"
direction = "read"
rename = "acme-user-me-get"
risk = "low"
idempotency = "idempotent"
"#;

/// **An unknown key inside a spec block still gets serde's error, in either spelling.**
///
/// The obvious way to accept both shapes is `#[serde(untagged)]`, and it is the wrong one: an
/// untagged enum buffers the input and reports `data did not match any variant of untagged enum`,
/// throwing away both the `deny_unknown_fields` key list and `toml`'s line, column and snippet.
/// "Shape errors are serde's, because serde's are better" is the loader's stated design, and a
/// mistyped `servicee` silently meaning "no service" is exactly the failure `deny_unknown_fields`
/// exists to stop.
#[test]
fn an_unknown_key_in_a_spec_block_is_still_named_in_both_spellings() {
    for spelling in ["[spec]", "[[spec]]"] {
        let definition = format!(
            "id = \"zendesk\"\nbase_url = \"https://acme.zendesk.com\"\n\n{spelling}\npath = \
             \"{PINNED}\"\nservicee = \"support\"\n"
        );
        let rendered = provider::load("providers/zendesk.toml", &definition)
            .expect_err("`servicee` is not a key any spec block accepts")
            .to_string();
        assert!(
            rendered.contains("servicee"),
            "{spelling} must name the offending key: {rendered}"
        );
        assert!(
            rendered.contains("service"),
            "{spelling} must list the keys that would have been valid: {rendered}"
        );
    }
}

/// **`[spec]` and a one-entry `[[spec]]` are one field in two spellings.**
///
/// The single table has to keep meaning exactly what it meant, because 53 shipped providers and two
/// golden error files are written against it. So the array form is the general case and the table is
/// its one-element instance — not a second code path that could drift from it.
#[test]
fn a_single_spec_table_and_a_one_entry_spec_array_compile_identically() {
    let table = with(
        "
[[patch.operations]]
select = \"showTicket\"
direction = \"read\"
rename = \"zendesk-ticket-show\"
risk = \"low\"
idempotency = \"idempotent\"
",
    );
    let array = table.replace("[spec]\n", "[[spec]]\n");
    assert_ne!(
        table, array,
        "the two spellings must really differ as bytes"
    );

    let from_table = load(&table);
    let from_array = provider::load_with_spec("providers/zendesk.toml", &array, &cache())
        .expect("`[[spec]]` with one entry is a valid provider file")
        .connector;

    assert_eq!(from_table.operations, from_array.operations);
    assert_eq!(
        from_table.provenance.specs, from_array.provenance.specs,
        "one document is one provenance entry either way"
    );
    assert_eq!(
        from_table.operations[0].service,
        connector_spec::DEFAULT_SERVICE
    );
}

/// **Both documents reach the IR, each as its own service.**
///
/// One document per connector was never decided, it was assumed, and the assumption costs babelforce
/// 389 of its 398 operations. A connector whose vendor splits its API across five documents does not
/// have to become five connectors.
#[test]
fn several_documents_each_become_one_service() {
    let connector = load_many(BOTH_GET_USER);

    let manager = connector
        .operation("acme-manager-user-get")
        .expect("the manager document's operation");
    assert_eq!(manager.service, "manager");
    assert_eq!(manager.path, "/api/v2/users/{user_id}");

    let user = connector
        .operation("acme-user-me-get")
        .expect("the user document's operation");
    assert_eq!(user.service, "user");
    assert_eq!(user.path, "/api/v3/me");
}

/// **The same `operationId` in two documents is two operations, not one won by file order.**
///
/// `getUser` genuinely exists in babelforce's `manager-2026-07-10` and in its `user-2026-06-25`, as
/// two different requests. Nothing downstream could tell a build that compiled the wrong one: the
/// op id would be right and the request would not.
#[test]
fn one_operation_id_in_two_documents_is_two_operations() {
    let connector = load_many(BOTH_GET_USER);
    assert_eq!(connector.operations.len(), 2);
    assert_ne!(
        connector.operations[0].path, connector.operations[1].path,
        "both patches were resolved against the same document"
    );
}

/// **A patch names its document as soon as there is more than one.**
///
/// Not a style rule: with two documents declaring `getUser`, an unqualified `select` has two
/// answers, and a loader that picked one would emit plausible, wrong Flux and exit 0.
#[test]
fn a_patch_that_names_no_service_is_refused_when_several_documents_are_declared() {
    let rendered = refuse_many(
        "
[[patch.operations]]
select = \"getUser\"
direction = \"read\"
rename = \"acme-user-get\"
risk = \"low\"
idempotency = \"idempotent\"
",
    );
    assert!(rendered.contains("getUser"), "{rendered}");
    assert!(rendered.contains("`service`"), "{rendered}");
    assert!(
        rendered.contains("manager") && rendered.contains("user"),
        "the refusal must list the documents that could have been meant: {rendered}"
    );
}

/// A `service` naming no declared document is loud, for the same reason a `select` naming no
/// `operationId` is: that is how a patch set rots underneath a re-vendor.
#[test]
fn a_patch_naming_a_service_no_document_declares_is_refused() {
    let rendered = refuse_many(
        "
[[patch.operations]]
service = \"task-automation\"
select = \"getUser\"
direction = \"read\"
rename = \"acme-user-get\"
risk = \"low\"
idempotency = \"idempotent\"
",
    );
    assert!(rendered.contains("task-automation"), "{rendered}");
    assert!(rendered.contains("manager"), "{rendered}");
}

/// Selecting one `operationId` twice **out of one document** is still the duplicate it always was —
/// the key widened to `(service, select)`, it did not disappear.
#[test]
fn selecting_one_operation_twice_from_one_document_is_still_refused() {
    let rendered = refuse_many(
        "
[[patch.operations]]
service = \"manager\"
select = \"getUser\"
direction = \"read\"
rename = \"acme-manager-user-get\"
risk = \"low\"
idempotency = \"idempotent\"

[[patch.operations]]
service = \"manager\"
select = \"getUser\"
direction = \"read\"
rename = \"acme-manager-user-fetch\"
risk = \"low\"
idempotency = \"idempotent\"
",
    );
    assert!(rendered.contains("more than once"), "{rendered}");
    assert!(rendered.contains("manager"), "{rendered}");
}

/// A document joins a service; it does not declare one. A `[[spec]] service` that no `[[services]]`
/// entry declares is refused, naming what the provider does declare.
#[test]
fn a_document_joining_an_undeclared_service_is_refused() {
    let definition = TWO_SPECS.replace("service = \"user\"", "service = \"users\"");
    let rendered = provider::load_with_spec("providers/acme.toml", &definition, &two_documents())
        .expect_err("`users` is not a declared service")
        .to_string();
    assert!(rendered.contains("users"), "{rendered}");
    assert!(rendered.contains("[[services]]"), "{rendered}");
}

/// Two documents joining one service is refused: a service is one name namespace, and two vendor
/// documents can declare one `operationId`.
#[test]
fn two_documents_may_not_join_one_service() {
    let definition = TWO_SPECS.replace(
        "path = \"specs/acme/user-2026-06-25.json\"\nservice = \"user\"",
        "path = \"specs/acme/user-2026-06-25.json\"\nservice = \"manager\"",
    );
    let rendered = provider::load_with_spec("providers/acme.toml", &definition, &two_documents())
        .expect_err("two documents cannot share a service")
        .to_string();
    assert!(rendered.contains("two documents"), "{rendered}");
    assert!(rendered.contains("manager"), "{rendered}");
}

/// **The documents' security models stay apart, and each operation resolves against the connector's
/// `default_auth` rather than against another document's declaration.**
///
/// Babelforce's manager document declares root `oauth2` with zero operation overrides, while
/// `task-automation` declares `bearerAuth`+`oauth2` on all 31 of its operations. One
/// `LoadedProvider::ingested` slot would have let whichever was folded in last speak for both.
///
/// `Operation::auth` is three-state: absent means *inherit `default_auth`*, `[]` means *this
/// operation needs no auth*. So the check is that an override stated on one service's patch does not
/// reach the other service's operation, and that the unstated one still inherits.
#[test]
fn one_documents_security_does_not_overwrite_the_others() {
    let loaded = provider::load_with_spec(
        "providers/acme.toml",
        &format!(
            "{TWO_SPECS}
[[patch.operations]]
service = \"manager\"
select = \"getUser\"
direction = \"read\"
rename = \"acme-manager-user-get\"
risk = \"low\"
idempotency = \"idempotent\"

[[patch.operations]]
service = \"user\"
select = \"getUser\"
direction = \"read\"
rename = \"acme-user-me-get\"
risk = \"low\"
idempotency = \"idempotent\"
auth = []
"
        ),
        &two_documents(),
    )
    .expect("two documents with two security models is a valid provider");

    let connector = &loaded.connector;
    assert_eq!(
        connector
            .operation("acme-manager-user-get")
            .expect("selected")
            .auth,
        None,
        "the manager operation states nothing, so it inherits the connector's `default_auth`"
    );
    assert_eq!(
        connector
            .operation("acme-user-me-get")
            .expect("selected")
            .auth,
        Some(Vec::new()),
        "the user operation's own statement must not have been taken from the other document"
    );

    // And the ingests are kept apart rather than merged, which is what makes the above structural
    // rather than incidental: each document stays available to inspect under its own service.
    assert_eq!(loaded.ingested.len(), 2);
    assert_eq!(
        loaded
            .ingested_for("manager")
            .expect("the manager document")
            .path,
        MANAGER_PATH
    );
    assert_eq!(
        loaded.ingested_for("user").expect("the user document").path,
        USER_PATH
    );
}

/// **Provenance is per document — one `sha256` each, so a drift check can say which one moved.**
///
/// A connector-wide hash cannot answer that question, and it is the only question a drift check is
/// asked. babelforce's five documents were pulled on two different dates and three of them publish
/// `info.version = "0.0.0-dev"`.
#[test]
fn each_document_carries_its_own_provenance_and_its_own_hash_is_checked() {
    let honest = TWO_SPECS.replace(
        "path = \"specs/acme/user-2026-06-25.json\"\nservice = \"user\"",
        &format!(
            "path = \"specs/acme/user-2026-06-25.json\"\nservice = \"user\"\nsha256 = \"{}\"\n\
             fetched_at = \"2026-06-25T09:37:13Z\"",
            connector_spec::sha256_hex(USER.as_bytes())
        ),
    );
    let loaded = provider::load_with_spec("providers/acme.toml", &honest, &two_documents())
        .expect("a declaration that matches its own document's bytes");

    let specs = &loaded.connector.provenance.specs;
    assert_eq!(specs.len(), 2, "one provenance entry per document");
    assert_eq!(specs[0].path, MANAGER_PATH);
    assert_eq!(specs[0].sha256, None);
    assert_eq!(
        specs[1].sha256.as_deref(),
        Some(connector_spec::sha256_hex(USER.as_bytes()).as_str())
    );
    assert_eq!(specs[1].fetched_at.as_deref(), Some("2026-06-25T09:37:13Z"));

    // The four connector-wide fields describe *a* spec, so with several documents no single value of
    // them is true and none is invented.
    assert_eq!(loaded.connector.provenance.spec_sha256, None);
    assert_eq!(loaded.connector.provenance.fetched_at, None);

    // And each hash is checked against **its own** document's bytes: the manager document's hash
    // declared on the user entry is a refusal that names only the document that did not match.
    let crossed = TWO_SPECS.replace(
        "path = \"specs/acme/user-2026-06-25.json\"\nservice = \"user\"",
        &format!(
            "path = \"specs/acme/user-2026-06-25.json\"\nservice = \"user\"\nsha256 = \"{}\"",
            connector_spec::sha256_hex(MANAGER.as_bytes())
        ),
    );
    let rendered = provider::load_with_spec("providers/acme.toml", &crossed, &two_documents())
        .expect_err("the manager document's hash is not the user document's")
        .to_string();
    assert!(rendered.contains(USER_PATH), "{rendered}");
    assert!(
        !rendered.contains(MANAGER_PATH),
        "only the document that moved is named: {rendered}"
    );
}

/// One document failing to resolve does not take the others with it: every problem in the file is
/// reported at once, which is the contract the rest of this loader keeps.
#[test]
fn a_pin_that_resolves_to_nothing_names_only_that_document() {
    let definition = TWO_SPECS.replace(
        "specs/acme/user-2026-06-25.json",
        "specs/acme/user-2025-01-01.json",
    );
    let rendered = provider::load_with_spec("providers/acme.toml", &definition, &two_documents())
        .expect_err("one of the two pins resolves to nothing")
        .to_string();
    assert!(
        rendered.contains("specs/acme/user-2025-01-01.json"),
        "{rendered}"
    );
    assert!(
        rendered.contains(MANAGER_PATH),
        "the refusal lists the cache, which holds the document that did resolve: {rendered}"
    );
}
