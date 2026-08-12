//! OpenAPI 3.x ingest — C-4.
//!
//! Two halves, deliberately kept apart:
//!
//! - **Fixture-driven**, over the trimmed real excerpts committed under `specs/`. Those are the
//!   documents this pipeline actually has to survive, and reading them off disk rather than
//!   embedding a copy here is what stops the thing under test from drifting away from the thing
//!   that ships.
//! - **Inline**, for the shapes a real excerpt cannot demonstrate without being deformed to carry
//!   them — an `openapi` version this ingest refuses, a `$ref` out of the document.
//!
//! The excerpts carry their own deliberate defects (`/api/v2/_ingest-fixture/…`), because "a real
//! vendor spec is never fully well-formed" is the half of this story that matters and a fixture that
//! is perfect proves nothing about it.

use std::path::{Path, PathBuf};

use connector_spec::openapi::{self, Ingested, SpecOperation};
use connector_spec::{BodyEncoding, HttpMethod};

/// `<repo root>/specs`, derived from this crate's manifest directory so the test does not depend on
/// the working directory a runner happens to use.
fn specs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("specs")
}

fn ingest_fixture(relative: &str) -> Ingested {
    let path = specs_dir().join(relative);
    let document = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    openapi::ingest(&document)
        .unwrap_or_else(|error| panic!("{} does not ingest: {error}", path.display()))
}

fn zendesk() -> Ingested {
    ingest_fixture("zendesk/2024-06-01-excerpt.json")
}

fn anthropic() -> Ingested {
    ingest_fixture("anthropic/2023-06-01-excerpt.yaml")
}

fn operation<'a>(ingested: &'a Ingested, id: &str) -> &'a SpecOperation {
    ingested.operation(id).unwrap_or_else(|| {
        panic!(
            "the document declares no `{id}`; it declares {:?}",
            ingested.operation_ids()
        )
    })
}

/// Every diagnostic, joined — what a failure prints so the reason is visible without a second run.
fn diagnostics(ingested: &Ingested) -> String {
    ingested
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------------------------
// The happy path, over both excerpts
// ---------------------------------------------------------------------------------------------

/// A 3.0 document and a 3.1 document both ingest, and the version is carried rather than assumed.
#[test]
fn both_openapi_3_0_and_3_1_documents_ingest() {
    assert_eq!(zendesk().version, "3.0.3");
    assert_eq!(anthropic().version, "3.1.0");
}

/// The YAML half. Every babelforce document is YAML, and its `responses:` keys are YAML *integers* —
/// a shape no JSON object can hold until they are coerced back to strings. A response schema
/// arriving here is the proof that coercion happened.
#[test]
fn a_yaml_document_ingests_including_its_integer_response_keys() {
    let anthropic = anthropic();
    let messages = operation(&anthropic, "messages_post");
    let response = messages
        .response_schema
        .as_ref()
        .expect("`responses: {200: …}` is an integer key in YAML and must still be found");
    assert_eq!(
        response
            .pointer("/properties/id/type")
            .and_then(|t| t.as_str()),
        Some("string"),
        "{response}"
    );
}

/// Path, query and header parameters all reach the IR in their own group, each carrying the vendor's
/// schema **verbatim** — keywords and all, because nothing here reinterprets a schema.
#[test]
fn parameters_land_in_their_request_position_with_their_schemas() {
    let zendesk = zendesk();

    let show = operation(&zendesk, "showTicket");
    assert_eq!(show.method, HttpMethod::Get);
    assert_eq!(show.path, "/api/v2/tickets/{ticket_id}");
    assert_eq!(show.description, "Show one ticket by id.");
    assert_eq!(show.params.path.len(), 1);
    let ticket_id = &show.params.path[0];
    assert_eq!(ticket_id.name, "ticket_id");
    assert!(ticket_id.required, "a path parameter is required");
    assert_eq!(
        ticket_id.schema,
        serde_json::json!({ "type": "integer", "format": "int64" }),
        "the vendor's `format` must survive"
    );

    let list = operation(&zendesk, "listTickets");
    let query: Vec<&str> = list
        .params
        .query
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    assert_eq!(query, vec!["page", "per_page"]);
    assert!(
        !list.params.query[0].required,
        "`page` states no `required`, and only a path parameter defaults to required"
    );
    assert_eq!(
        list.params.query[1].schema,
        serde_json::json!({ "type": "integer", "maximum": 100 })
    );
    let headers: Vec<&str> = list
        .params
        .header
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    assert_eq!(headers, vec!["X-Request-Id"]);
}

/// A path item's own `parameters` are inherited by every operation under it — the `ticket_id` that
/// `showTicket` and `deleteTicket` share is declared once, on the path.
#[test]
fn a_path_items_parameters_reach_every_operation_under_it() {
    let zendesk = zendesk();
    for id in ["showTicket", "deleteTicket"] {
        let operation = operation(&zendesk, id);
        assert_eq!(
            operation
                .params
                .path
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>(),
            vec!["ticket_id"],
            "`{id}` did not inherit the path item's parameter"
        );
    }
}

/// An object request body becomes **named** body fields, one per top-level property, with `required`
/// read off the schema's own list. That is what reaches a model as separate arguments.
#[test]
fn an_object_request_body_becomes_named_body_parameters() {
    let zendesk = zendesk();
    let create = operation(&zendesk, "createTicket");
    assert_eq!(create.method, HttpMethod::Post);
    assert_eq!(create.params.body_encoding, BodyEncoding::Json);
    assert!(
        create.params.body_schema.is_none(),
        "an object body is named fields, not a free-form schema"
    );

    let fields: Vec<(&str, bool)> = create
        .params
        .body
        .iter()
        .map(|param| (param.name.as_str(), param.required))
        .collect();
    assert_eq!(fields, vec![("async", false), ("ticket", true)]);

    // Only the top level is expanded: `ticket` stays one parameter carrying the whole object, so the
    // vendor's nesting is not presented as if it were the caller's.
    let ticket = &create.params.body[1].schema;
    assert_eq!(
        ticket.pointer("/properties/comment/properties/body/type"),
        Some(&serde_json::json!("string")),
        "{ticket}"
    );
}

/// `servers` produce the base URL with its templating **preserved**, because `{subdomain}` is a
/// per-tenant value a host substitutes — filling in the vendor's `default` would bake one example
/// account into every connector.
#[test]
fn servers_carry_their_templating_and_their_variables() {
    let zendesk = zendesk();
    assert_eq!(zendesk.base_url(), Some("https://{subdomain}.zendesk.com"));

    let server = &zendesk.servers[0];
    let subdomain = server
        .variables
        .get("subdomain")
        .expect("the document declares the variable its URL templates");
    assert_eq!(subdomain.default, "example");
    assert!(
        !server.url.contains("example"),
        "the default must be recorded, never substituted: {}",
        server.url
    );
}

// ---------------------------------------------------------------------------------------------
// `$ref` resolution
// ---------------------------------------------------------------------------------------------

/// A `$ref` is resolved wherever it appears — as a whole parameter, and at any depth inside a
/// schema — so nothing downstream has to know the document a schema came from.
#[test]
fn refs_resolve_including_nested_and_repeated_ones() {
    let zendesk = zendesk();

    // A parameter that *is* a `$ref`.
    let list = operation(&zendesk, "listTickets");
    let page = &list.params.query[0];
    assert_eq!(page.description, "Which page to fetch.");
    assert_eq!(
        page.schema,
        serde_json::json!({ "type": "integer", "minimum": 1 })
    );

    // Nested: TicketResponse -> Ticket -> Via -> ViaSource, three hops down one chain.
    let show = operation(&zendesk, "showTicket");
    let response = show.response_schema.as_ref().expect("a 200 JSON schema");
    assert_eq!(
        response.pointer("/properties/ticket/properties/via/properties/source/properties/rel/type"),
        Some(&serde_json::json!("string")),
        "a nested `$ref` chain did not resolve: {response}"
    );

    // Repeated: `Ticket` is reached from two different responses, and resolves both times. A
    // resolver that mistook a second visit for a cycle would drop one of them.
    let listed = operation(&zendesk, "listTickets")
        .response_schema
        .as_ref()
        .expect("a 200 JSON schema");
    assert_eq!(
        listed.pointer("/properties/tickets/items/properties/via/properties/channel/type"),
        Some(&serde_json::json!("string")),
        "a `$ref` repeated across two responses did not resolve: {listed}"
    );
}

/// A response schema that contains itself keeps one useful level and explicitly admits any deeper
/// continuation. It therefore cannot expand forever and does not invent a finite maximum depth.
#[test]
fn a_cyclic_response_ref_is_bounded_rather_than_expanded_forever() {
    let zendesk = zendesk();
    let response = operation(&zendesk, "showOrganization")
        .response_schema
        .as_ref()
        .expect("the recursive response remains useful");
    assert_eq!(
        response.pointer("/properties/id/type"),
        Some(&serde_json::json!("integer"))
    );
    assert_eq!(
        response.pointer("/properties/children/items"),
        Some(&serde_json::json!(true))
    );
}

/// Recursive response models are useful even though this IR cannot carry local references. The
/// resolver keeps one concrete level and bounds the recursive continuation with JSON Schema's
/// `true` schema; request schemas remain exact and therefore keep the stricter refusal above.
#[test]
fn a_recursive_response_is_bounded_without_dropping_the_operation() {
    let ingested = openapi::ingest(
        r##"{
          "openapi": "3.0.3",
          "servers": [{"url": "https://api.acme.example"}],
          "paths": {
            "/nodes": {
              "get": {
                "operationId": "listNodes",
                "responses": {
                  "200": {
                    "description": "ok",
                    "content": {
                      "application/json": {
                        "schema": {"$ref": "#/components/schemas/Node"}
                      }
                    }
                  }
                }
              }
            }
          },
          "components": {
            "schemas": {
              "Node": {
                "type": "object",
                "properties": {
                  "id": {"type": "string"},
                  "children": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/Node"}
                  }
                }
              }
            }
          }
        }"##,
    )
    .expect("a well-formed recursive response document");

    let operation = operation(&ingested, "listNodes");
    let response = operation
        .response_schema
        .as_ref()
        .expect("the bounded response remains available");
    assert_eq!(
        response.pointer("/properties/id/type"),
        Some(&serde_json::json!("string"))
    );
    assert_eq!(
        response.pointer("/properties/children/items"),
        Some(&serde_json::json!(true)),
        "the recursive continuation is explicitly unconstrained rather than fabricated: {response}"
    );
}

#[test]
fn a_recursive_request_remains_an_exact_contract_and_is_refused() {
    let ingested = openapi::ingest(
        r##"{
          "openapi": "3.0.3",
          "servers": [{"url": "https://api.acme.example"}],
          "paths": {
            "/nodes": {
              "post": {
                "operationId": "createNode",
                "requestBody": {
                  "content": {
                    "application/json": {
                      "schema": {"$ref": "#/components/schemas/Node"}
                    }
                  }
                }
              }
            }
          },
          "components": {
            "schemas": {
              "Node": {
                "type": "object",
                "properties": {
                  "child": {"$ref": "#/components/schemas/Node"}
                }
              }
            }
          }
        }"##,
    )
    .expect("a well-formed recursive request document");

    assert!(ingested.operation("createNode").is_none());
    assert!(
        diagnostics(&ingested).contains("$ref` cycle"),
        "the executable request contract must not be truncated:\n{}",
        diagnostics(&ingested)
    );
}

// ---------------------------------------------------------------------------------------------
// The diagnostic path — the important half
// ---------------------------------------------------------------------------------------------

/// One bad endpoint costs that endpoint and nothing else. Every deliberate defect in the Zendesk
/// excerpt is reported by name, and the five sound operations beside them still ingest.
#[test]
fn a_malformed_endpoint_is_a_diagnostic_naming_it_rather_than_a_failed_ingest() {
    let zendesk = zendesk();

    let mut ingested = zendesk.operation_ids();
    ingested.sort_unstable();
    assert_eq!(
        ingested,
        vec![
            "createTicket",
            "deleteTicket",
            "listTickets",
            "showOrganization",
            "showTicket",
        ],
        "the sound operations must survive their neighbours' defects"
    );

    for (location, expected) in [
        ("GET /api/v2/_ingest-fixture/untyped", "no `schema`"),
        (
            "POST /api/v2/_ingest-fixture/uploads",
            "multipart/form-data",
        ),
        ("GET /api/v2/_ingest-fixture/anonymous", "operationId"),
    ] {
        let reported = zendesk
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.location == location)
            .unwrap_or_else(|| panic!("nothing reported `{location}`:\n{}", diagnostics(&zendesk)));
        assert!(
            reported.problem.contains(expected),
            "the diagnostic for `{location}` must say why: {reported}"
        );
    }
}

/// A body in a media type the IR cannot express skips the **whole operation**.
///
/// The tempting alternative — ingest it without its body — produces a `POST` that quietly sends
/// nothing, which is indistinguishable from a legitimately bodiless write and would ship as a
/// working connector. `BodyEncoding` is `json | form`; `multipart/form-data` is a known blocker
/// (`docs/designs/spec-front-end.md`), not a surprise to paper over.
#[test]
fn a_body_the_ir_cannot_express_skips_the_operation_rather_than_dropping_the_body() {
    let zendesk = zendesk();
    assert!(
        zendesk.operation("fixtureMultipartUpload").is_none(),
        "an operation whose body cannot be expressed must not reach the IR bodiless"
    );
}

/// An operation under `options` or `trace` has no representation in the IR at all, and earns a
/// diagnostic rather than vanishing.
///
/// Silence here would send an author who selected it to "names no `operationId`" about an operation
/// they can read in the document, with nothing pointing at the method as the cause.
#[test]
fn a_method_the_ir_cannot_spell_is_reported_rather_than_dropped_silently() {
    let ingested = openapi::ingest(
        r#"{
          "openapi": "3.0.3",
          "servers": [{"url": "https://api.acme.example"}],
          "paths": {
            "/things": {
              "get": {"operationId": "listThings"},
              "options": {"operationId": "thingsPreflight"},
              "trace": {"operationId": "thingsTrace"}
            }
          }
        }"#,
    )
    .expect("a well-formed document");

    assert_eq!(
        ingested.operation_ids(),
        vec!["listThings"],
        "the sound operation beside them still ingests"
    );
    let reported: Vec<&str> = ingested
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.location.as_str())
        .collect();
    assert_eq!(reported, vec!["OPTIONS /things", "TRACE /things"]);
}

/// A cookie parameter skips the **whole operation**, exactly as an unrepresentable body does.
///
/// `ParamSet` has no cookie position, so publishing the operation without it would ship a request
/// that quietly stopped sending something the vendor declared — the same failure shape the module
/// already refuses for `multipart/form-data`, and the argument does not weaken by moving from a body
/// to a parameter.
#[test]
fn a_cookie_parameter_skips_the_operation_rather_than_being_dropped_from_it() {
    let ingested = openapi::ingest(
        r#"{
          "openapi": "3.0.3",
          "servers": [{"url": "https://api.acme.example"}],
          "paths": {
            "/things": {
              "get": {
                "operationId": "listThings",
                "parameters": [
                  {"name": "session", "in": "cookie", "schema": {"type": "string"}}
                ]
              }
            }
          }
        }"#,
    )
    .expect("a well-formed document");

    assert!(
        ingested.operations.is_empty(),
        "the operation must not be published without a parameter the vendor requires"
    );
    let reported = &ingested.diagnostics[0];
    assert_eq!(reported.location, "GET /things");
    assert!(reported.problem.contains("cookie"), "{reported}");
}

/// A document that is not an OpenAPI 3.x document at all is an [`Err`], not a diagnostic — nothing
/// useful can follow, so there is nothing to degrade to.
#[test]
fn a_document_this_ingest_cannot_read_is_a_whole_document_error() {
    let refused = [
        ("{\"swagger\": \"2.0\", \"paths\": {}}", "openapi"),
        ("{\"openapi\": \"4.0.0\", \"paths\": {}}", "4.0.0"),
        ("[]", "not a mapping"),
        ("\t- : : :\n  \x7f", "neither JSON nor YAML"),
    ];
    for (document, expected) in refused {
        let error = openapi::ingest(document)
            .map(|ingested| ingested.operation_ids().join(", "))
            .expect_err("this is not a document ingest can read");
        let rendered = error.to_string();
        assert!(
            rendered.contains(expected),
            "the refusal must say what is wrong: {rendered}"
        );
    }
}

/// An external `$ref` cannot be followed, because this crate touches no filesystem at all. The
/// operation is skipped and the diagnostic says so rather than resolving to something plausible.
#[test]
fn an_external_ref_is_refused_rather_than_followed() {
    let ingested = openapi::ingest(
        r#"{
          "openapi": "3.0.3",
          "servers": [{"url": "https://api.acme.example"}],
          "paths": {
            "/things": {
              "get": {
                "operationId": "listThings",
                "parameters": [{"$ref": "common.yaml#/components/parameters/Page"}]
              }
            }
          }
        }"#,
    )
    .expect("the document itself is well-formed");

    assert!(ingested.operations.is_empty());
    let reported = &ingested.diagnostics[0];
    assert_eq!(reported.location, "GET /things");
    assert!(
        reported.problem.contains("common.yaml"),
        "the diagnostic must quote the reference it could not follow: {reported}"
    );
}

/// A missing whole section degrades to a diagnostic too — an absent `servers` is legal OpenAPI, and
/// silence about it would read as agreement with whatever `base_url` the provider file states.
#[test]
fn a_missing_section_is_a_diagnostic_naming_it() {
    let ingested = openapi::ingest(r#"{"openapi": "3.1.0"}"#).expect("a legal, empty 3.1 document");
    let reported: Vec<&str> = ingested
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.location.as_str())
        .collect();
    assert_eq!(reported, vec!["servers", "paths"]);
    assert!(ingested.operations.is_empty());
    assert_eq!(ingested.base_url(), None);
}

/// Two operations sharing one `operationId` would make a `select` naming it mean two different
/// requests. The first wins and the collision is reported, rather than the last silently replacing
/// the first.
#[test]
fn a_duplicate_operation_id_is_reported_rather_than_resolved_by_position() {
    let ingested = openapi::ingest(
        r#"{
          "openapi": "3.0.3",
          "servers": [{"url": "https://api.acme.example"}],
          "paths": {
            "/a": {"get": {"operationId": "collide", "summary": "The first."}},
            "/b": {"get": {"operationId": "collide", "summary": "The second."}}
          }
        }"#,
    )
    .expect("a well-formed document");

    assert_eq!(ingested.operation_ids(), vec!["collide"]);
    assert_eq!(ingested.operation("collide").unwrap().path, "/a");
    let reported = &ingested.diagnostics[0];
    assert_eq!(reported.location, "GET /b");
    assert!(reported.problem.contains("more than once"), "{reported}");
}

// ---------------------------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------------------------

/// Ingesting the same bytes twice produces the same value. `connectors.lock` hashes the IR, so any
/// leaked iteration order would surface as phantom drift on every build.
#[test]
fn ingest_is_deterministic() {
    assert_eq!(zendesk(), zendesk());
    assert_eq!(anthropic(), anthropic());
}

/// Operation order is a property of the document's content, not of its key order: paths are walked
/// sorted, and methods in a fixed order. Two documents that differ only in how the vendor typed them
/// ingest identically.
#[test]
fn operation_order_does_not_depend_on_the_documents_key_order() {
    let one = openapi::ingest(
        r#"{"openapi":"3.0.3","servers":[{"url":"https://a.example"}],"paths":{
             "/z":{"get":{"operationId":"z"}},
             "/a":{"post":{"operationId":"aPost"},"get":{"operationId":"aGet"}}}}"#,
    )
    .expect("a well-formed document");
    let other = openapi::ingest(
        r#"{"openapi":"3.0.3","servers":[{"url":"https://a.example"}],"paths":{
             "/a":{"get":{"operationId":"aGet"},"post":{"operationId":"aPost"}},
             "/z":{"get":{"operationId":"z"}}}}"#,
    )
    .expect("a well-formed document");

    assert_eq!(one.operation_ids(), vec!["aGet", "aPost", "z"]);
    assert_eq!(one, other);
}
