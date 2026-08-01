//! The emitter contract: one IR [`Operation`] becomes one formatted Flux `op` declaration.
//!
//! Five properties are pinned here, and they are the ones every later codegen story inherits:
//!
//! 1. **Golden files.** `tests/golden/*.flux` pin the exact generated text for real operations drawn
//!    from `docs/designs/provider-operation-inventory.md`. A codegen change therefore surfaces as a
//!    reviewable diff instead of as a silent behavior change. Re-record them with
//!    `UPDATE_GOLDEN=1 cargo test -p connector-flux`, then *read the diff*.
//! 2. **Canonical formatting.** The emitted text must parse cleanly and be a fixed point of
//!    flux-lang's own formatter — see [`emitted_text_is_a_fixed_point_of_the_flux_formatter`]. This
//!    is what "build AST nodes, never string templates" buys, and it is asserted rather than assumed.
//! 3. **Name safety.** A vendor parameter name that is not a Flux identifier (babelforce's
//!    `time.start`) travels as an explicit wire-name/symbol-name pair — the symbol is spellable, the
//!    query string still carries the vendor's spelling.
//! 4. **A non-2xx is data.** `http.request` returns a 404 with its status and *succeeds*, so nothing
//!    generated here asserts on the response — see
//!    [`a_non_2xx_response_is_data_not_an_op_failure`]. The response is bound and returned whole.
//! 5. **A write may not carry a read's metadata.** flux's approval gate reads `risk` and
//!    `idempotency`, so the emitter refuses a state-changing method that declares `risk = "low"`, or
//!    a `POST`/`PATCH` that claims to be idempotent.
//!
//! # A note on the fixture ids
//!
//! The inventory names these operations `zendesk.ticket.comment.list` and friends, and so does
//! [connector-pipeline.md](../../../docs/designs/connector-pipeline.md). **A dotted name cannot be
//! a Flux `op` *declaration* name** — `flux_lang`'s `decl_name` grammar admits only ASCII
//! alphanumerics, `_` and `-`, and flux's own composite loader rejects the rest as "not
//! filename-safe" (`../flux/crates/flux-flow/src/composites.rs:340`). Calls accept dots; the
//! declaration that would define the op does not.
//!
//! Choosing the real public form is [C-23](../../../docs/stories/C-23-operation-naming-contract.md)'s
//! job, so these fixtures use a declarable kebab rendering and the emitter *refuses* an id it cannot
//! spell (see [`an_id_flux_cannot_declare_is_refused`]) rather than rewriting one behind the
//! caller's back.

use connector_flux::emit_operation;
use connector_spec::{
    Connector, ErrorEnvelope, HttpMethod, Idempotency, Operation, Param, ParamSet, Provenance,
    Quirks, Risk, DEFAULT_SERVICE,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// Fixtures — real operations, cited to the inventory
// ---------------------------------------------------------------------------

fn param(name: &str, description: &str, required: bool, schema: serde_json::Value) -> Param {
    Param {
        name: name.to_string(),
        wire: None,
        description: description.to_string(),
        required,
        schema,
    }
}

/// A parameter whose vendor spelling differs from its caller-facing name — a body field's JSON path
/// (`ticket.comment.body`) or a query alias (`req_id` → `requester_id`). One field, both cases; see
/// [`connector_spec::Param::wire`].
fn wired(
    name: &str,
    wire: &str,
    description: &str,
    required: bool,
    schema: serde_json::Value,
) -> Param {
    Param {
        wire: Some(wire.to_string()),
        ..param(name, description, required, schema)
    }
}

fn connector(id: &str, base_url: &str, operation: Operation) -> Connector {
    Connector {
        id: id.to_string(),
        authority: None,
        runtime: connector_spec::Runtime::Http,
        api_version: None,
        services: Vec::new(),
        vendor: String::new(),
        base_url: base_url.to_string(),
        description: String::new(),
        auth: Vec::new(),
        default_auth: Vec::new(),
        operations: vec![operation],
        events: Vec::new(),
        channels: Vec::new(),
        config: Vec::new(),
        verify: None,
        graphs: Vec::new(),
        provenance: Provenance::default(),
    }
}

/// `zendesk-ticket-comment-list` — a path parameter plus two optional query parameters.
/// Inventory §3.2 op 4 (`../flux/plugins/zendesk/src/main.rs:42-51`).
fn zendesk_comment_list() -> Connector {
    connector(
        "zendesk",
        "https://example.zendesk.com",
        Operation {
            id: "zendesk-ticket-comment-list".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Get,
            path: "/api/v2/tickets/{ticket_id}/comments.json".to_string(),
            description: "List one Zendesk ticket's comments.".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
            repeatable_because: None,
            auth: None,
            params: ParamSet {
                path: vec![param(
                    "ticket_id",
                    "The ticket to read comments from.",
                    true,
                    json!({"type": "integer", "format": "int64", "minimum": 1}),
                )],
                query: vec![
                    param(
                        "page",
                        "Page number, from 1.",
                        false,
                        json!({"type": "integer", "minimum": 1}),
                    ),
                    param(
                        "per_page",
                        "Page size, capped at 100 by Zendesk.",
                        false,
                        json!({"type": "integer", "minimum": 1, "maximum": 100}),
                    ),
                ],
                header: Vec::new(),
                body: Vec::new(),
                body_schema: None,
                ..ParamSet::default()
            },
            response_schema: None,
            quirks: Default::default(),
        },
    )
}

/// `zendesk-ticket-search` — a **required** query parameter alongside optional ones, and no path
/// parameter. Inventory §3.2 op 2 (`../flux/plugins/zendesk/src/main.rs:23-31`).
fn zendesk_ticket_search() -> Connector {
    connector(
        "zendesk",
        "https://example.zendesk.com",
        Operation {
            id: "zendesk-ticket-search".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Get,
            path: "/api/v2/search.json".to_string(),
            description: "Search Zendesk tickets with Zendesk search syntax.".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
            repeatable_because: None,
            auth: None,
            params: ParamSet {
                path: Vec::new(),
                query: vec![
                    param(
                        "query",
                        "A Zendesk search expression, e.g. `type:ticket status:new`.",
                        true,
                        json!({"type": "string"}),
                    ),
                    param(
                        "page",
                        "Page number, from 1.",
                        false,
                        json!({"type": "integer", "minimum": 1}),
                    ),
                    param(
                        "per_page",
                        "Page size, capped at 100 by Zendesk.",
                        false,
                        json!({"type": "integer", "minimum": 1, "maximum": 100}),
                    ),
                ],
                header: Vec::new(),
                body: Vec::new(),
                body_schema: None,
                ..ParamSet::default()
            },
            response_schema: None,
            quirks: Default::default(),
        },
    )
}

/// `babelforce-call-list` — the name-safety fixture. `time.start` / `time.end` carry dots, which
/// are **not** identifier-safe in Flux (inventory §6.5, `manager-0.7.0.openapi.json:2472`), and
/// `agentId` is a `oneOf` scalar-or-array that Flux's `TypeRef` cannot express.
fn babelforce_call_list() -> Connector {
    connector(
        "babelforce",
        "https://services.babelforce.com",
        Operation {
            id: "babelforce-call-list".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Get,
            path: "/api/v2/calls/reporting".to_string(),
            description: "List and filter calls, in the reporting view.".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
            repeatable_because: None,
            auth: None,
            params: ParamSet {
                path: Vec::new(),
                query: vec![
                    param("page", "Page number.", false, json!({"type": "integer"})),
                    param("max", "Page size.", false, json!({"type": "integer"})),
                    param(
                        "agentId",
                        "Filter by agent.",
                        false,
                        json!({"oneOf": [
                            {"type": "string", "format": "uuid"},
                            {"type": "array", "items": {"type": "string", "format": "uuid"}}
                        ]}),
                    ),
                    param(
                        "time.start",
                        "Window start, as a unix timestamp.",
                        false,
                        json!({"type": "integer"}),
                    ),
                    param(
                        "time.end",
                        "Window end, as a unix timestamp.",
                        false,
                        json!({"type": "integer"}),
                    ),
                    param("q", "Free-text search.", false, json!({"type": "string"})),
                ],
                header: Vec::new(),
                body: Vec::new(),
                body_schema: None,
                ..ParamSet::default()
            },
            response_schema: None,
            quirks: Default::default(),
        },
    )
}

/// `zendesk-test` — the floor of the shape: no parameters at all, so no `sep` and no guards.
/// Inventory §3.2 op 1 (`../flux/plugins/zendesk/src/main.rs:19`).
fn zendesk_test() -> Connector {
    connector(
        "zendesk",
        "https://example.zendesk.com",
        Operation {
            id: "zendesk-test".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Get,
            path: "/api/v2/users/me.json".to_string(),
            description: "Verify Zendesk credentials by fetching the authenticated user."
                .to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
            repeatable_because: None,
            auth: None,
            params: ParamSet::default(),
            response_schema: None,
            quirks: Default::default(),
        },
    )
}

/// `freshdesk-ticket-note-add` — the **write** fixture: a POST whose payload travels in a JSON
/// body, alongside a path parameter. Transcribed from `providers/freshdesk.toml` so the golden is
/// the text a real build emits. Freshdesk sends `content-type: application/json` on every request
/// (inventory §4.3, `yml:37-50`), which is where the emitter's one static header comes from.
/// Inventory §4.2 op 6 (`yml:307-348`).
fn freshdesk_note_add() -> Connector {
    connector(
        "freshdesk",
        "https://example.freshdesk.com/api/v2",
        Operation {
            id: "freshdesk-ticket-note-add".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Post,
            path: "/tickets/{id}/notes".to_string(),
            description:
                "Add a note to a ticket; the note is private unless explicitly made public"
                    .to_string(),
            risk: Risk::Medium,
            idempotency: Idempotency::NonIdempotent,
            repeatable_because: None,
            auth: None,
            params: ParamSet {
                path: vec![param(
                    "id",
                    "Id of the ticket",
                    true,
                    json!({"type": "integer"}),
                )],
                query: Vec::new(),
                header: Vec::new(),
                body: vec![
                    param(
                        "body",
                        "Note body",
                        true,
                        json!({"type": "string", "minLength": 1}),
                    ),
                    param(
                        "private",
                        "Whether the note is hidden from end users",
                        false,
                        json!({"type": "boolean", "default": true}),
                    ),
                    param(
                        "incoming",
                        "Set true when the note originates from an external system",
                        false,
                        json!({"type": "boolean", "default": true}),
                    ),
                    param(
                        "notify_emails",
                        "Addresses to notify about the note",
                        false,
                        json!({"type": "array", "items": {"type": "string", "format": "email"}}),
                    ),
                ],
                body_schema: None,
                ..ParamSet::default()
            },
            response_schema: None,
            quirks: Default::default(),
        },
    )
}

/// `zendesk-ticket-show` — the **error-envelope** fixture. Zendesk answers a missing ticket with
/// `404` and a body of `{"error": "RecordNotFound", "description": "Not found"}`; `http.request`
/// hands that back as a *result*, so the op contract has to say where the vendor hid the message.
/// Inventory §3.2 op 3 (`../flux/plugins/zendesk/src/main.rs:35-38`).
fn zendesk_ticket_show() -> Connector {
    connector(
        "zendesk",
        "https://example.zendesk.com",
        Operation {
            id: "zendesk-ticket-show".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Get,
            path: "/api/v2/tickets/{ticket_id}.json".to_string(),
            description: "Show one ticket".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
            repeatable_because: None,
            auth: None,
            params: ParamSet {
                path: vec![param(
                    "ticket_id",
                    "The ticket id",
                    true,
                    json!({"type": "integer", "format": "uint64", "minimum": 1}),
                )],
                ..ParamSet::default()
            },
            response_schema: None,
            quirks: Quirks {
                error_envelope: Some(ErrorEnvelope {
                    message_pointer: "/description".to_string(),
                    code_pointer: Some("/error".to_string()),
                }),
                ..Quirks::default()
            },
        },
    )
}

fn emit_only_operation(connector: &Connector) -> String {
    emit_operation(connector, &connector.operations[0])
        .expect("every fixture operation is inside the emitter's slice")
}

// ---------------------------------------------------------------------------
// Golden files
// ---------------------------------------------------------------------------

/// Compare `actual` against `tests/golden/<name>`, or rewrite it when `UPDATE_GOLDEN` is set.
fn assert_golden(name: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden dir")).expect("create golden dir");
        std::fs::write(&path, actual).expect("write golden");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read golden {}: {e}\nre-record with UPDATE_GOLDEN=1 cargo test -p connector-flux",
            path.display()
        )
    });
    assert_eq!(
        actual,
        expected,
        "generated Flux drifted from {}\n--- generated ---\n{actual}\n--- end ---\n\
         re-record with UPDATE_GOLDEN=1 cargo test -p connector-flux, then read the diff",
        path.display()
    );
}

#[test]
fn golden_zendesk_ticket_comment_list() {
    assert_golden(
        "zendesk-ticket-comment-list.flux",
        &emit_only_operation(&zendesk_comment_list()),
    );
}

#[test]
fn golden_zendesk_ticket_search() {
    assert_golden(
        "zendesk-ticket-search.flux",
        &emit_only_operation(&zendesk_ticket_search()),
    );
}

#[test]
fn golden_zendesk_test() {
    assert_golden("zendesk-test.flux", &emit_only_operation(&zendesk_test()));
}

#[test]
fn golden_babelforce_call_list() {
    assert_golden(
        "babelforce-call-list.flux",
        &emit_only_operation(&babelforce_call_list()),
    );
}

/// A POST carrying a JSON request body — C-9's headline shape.
#[test]
fn golden_freshdesk_ticket_note_add() {
    assert_golden(
        "freshdesk-ticket-note-add.flux",
        &emit_only_operation(&freshdesk_note_add()),
    );
}

/// An operation whose vendor hides its errors inside a non-2xx body.
#[test]
fn golden_zendesk_ticket_show() {
    assert_golden(
        "zendesk-ticket-show.flux",
        &emit_only_operation(&zendesk_ticket_show()),
    );
}

// ---------------------------------------------------------------------------
// The load-bearing properties
// ---------------------------------------------------------------------------

/// **The convention, asserted.** Emitted text must parse cleanly *and* be a fixed point of
/// flux-lang's own CST formatter: `format_module` re-prints the parsed tree and only returns text it
/// has proved re-parses to the same module, so `Some(emitted)` means "already canonical". Anything
/// a string template got subtly wrong — a stray space, a wrong indent, an unquoted metadata value —
/// shows up here as either `None` (does not parse) or a different string.
#[test]
fn emitted_text_is_a_fixed_point_of_the_flux_formatter() {
    for connector in [
        zendesk_test(),
        zendesk_comment_list(),
        zendesk_ticket_search(),
        babelforce_call_list(),
        freshdesk_note_add(),
        zendesk_ticket_show(),
        headered_operation(),
        constant_body_operation(),
        zendesk_comment_add(),
        babelforce_session_set(),
    ] {
        let emitted = emit_only_operation(&connector);
        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "emitted Flux for `{}` does not parse: {:?}\n{emitted}",
            connector.operations[0].id,
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str()),
            "the flux formatter would reformat the emitted module for `{}`",
            connector.operations[0].id
        );
    }
}

/// The emitted declaration loads back through flux-lang's own module loader as a composite `op`
/// carrying the metadata the approval gate reads. `risk`/`idempotency` come from the IR, never from
/// a default: the IR makes both mandatory precisely so they cannot be decided by silence.
#[test]
fn emitted_op_reloads_with_its_metadata_intact() {
    let connector = zendesk_comment_list();
    let emitted = emit_only_operation(&connector);

    let module = flux_lang::program::Module::parse_str(&emitted).expect("emitted Flux must parse");
    let program = module.program().expect("an `op` declaration is a program");
    assert_eq!(program.ops.len(), 1);

    let op = &program.ops[0];
    assert_eq!(op.name, "zendesk-ticket-comment-list");
    assert_eq!(op.meta.description, "List one Zendesk ticket's comments.");
    assert_eq!(serde_json::to_value(op.meta.risk).unwrap(), json!("low"));
    assert_eq!(
        serde_json::to_value(op.meta.idempotency).unwrap(),
        json!("idempotent")
    );
    assert_eq!(
        serde_json::to_value(&op.meta.effects).unwrap(),
        json!(["network"])
    );
    assert!(op.meta.expose, "`expose true` is what makes it an LLM tool");
    assert_eq!(
        op.returns.as_ref().map(|t| t.label()),
        Some("Any".to_string())
    );
}

/// A write is emitted as a write. The gate reads these two fields, so a POST must not inherit a
/// GET's `low`/`idempotent` by accident.
#[test]
fn risk_and_idempotency_are_taken_from_the_ir_not_assumed() {
    let mut connector = zendesk_comment_list();
    connector.operations[0].risk = Risk::Destructive;
    connector.operations[0].idempotency = Idempotency::NonIdempotent;
    connector.operations[0].method = HttpMethod::Delete;

    let emitted = emit_only_operation(&connector);
    assert!(
        emitted.contains("risk \"destructive\""),
        "expected a destructive risk in:\n{emitted}"
    );
    assert!(
        emitted.contains("idempotency \"non_idempotent\""),
        "expected a non-idempotent marker in:\n{emitted}"
    );
    assert!(
        emitted.contains("method: \"DELETE\""),
        "expected the IR's method on the request in:\n{emitted}"
    );
}

/// Path parameters are substituted by **symbol** name while the URL keeps the vendor's path shape.
#[test]
fn path_parameters_substitute_into_the_url() {
    let emitted = emit_only_operation(&zendesk_comment_list());
    assert!(
        emitted.contains(r#"url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")"#),
        "expected the path template interpolated into $url in:\n{emitted}"
    );
}

/// Query parameters assemble onto the request: a required one is always present, an optional one is
/// guarded so an unsupplied filter never reaches the vendor as an empty `key=`.
#[test]
fn query_parameters_assemble_into_the_request() {
    let emitted = emit_only_operation(&zendesk_ticket_search());
    assert!(
        emitted.contains(r#"url = fmt("{base}/api/v2/search.json?query={query}")"#),
        "a required query parameter belongs in the base URL:\n{emitted}"
    );
    assert!(
        emitted.contains("when page\n") && emitted.contains(r#"fmt("{url}{sep}page={page}")"#),
        "an optional query parameter must be guarded:\n{emitted}"
    );
}

/// A vendor parameter name containing a dot is **not** identifier-safe in Flux. The op declares the
/// mapped symbol, and the query string still carries the vendor's own spelling — nothing is mangled
/// silently in either direction.
#[test]
fn dotted_vendor_names_map_to_flux_symbols_without_losing_the_wire_name() {
    let emitted = emit_only_operation(&babelforce_call_list());
    assert!(
        emitted.contains("time_start: Number") && emitted.contains("time_end: Number"),
        "the declared params must be spellable Flux symbols:\n{emitted}"
    );
    assert!(
        emitted.contains(r#"fmt("{url}{sep}time.start={time_start}")"#),
        "the query string must keep the vendor's wire name:\n{emitted}"
    );
    assert!(
        !emitted.contains("{time.start}"),
        "a dotted symbol reference would silently reparse as field access:\n{emitted}"
    );
}

/// Flux's `TypeRef` has four scalars, a homogeneous list and named types; a `oneOf` union is none of
/// those, so it lands on the documented `Any` fallback rather than being guessed at.
#[test]
fn a_shape_flux_cannot_express_falls_back_to_any() {
    let emitted = emit_only_operation(&babelforce_call_list());
    assert!(
        emitted.contains("agentId: Any"),
        "a scalar-or-array union must degrade to Any:\n{emitted}"
    );
}

/// The dotted op name the pipeline design assumes is not spellable as a Flux **declaration**. The
/// emitter says so instead of quietly rewriting it — a silent rename is exactly the failure C-23
/// exists to prevent, and a rewritten name would parse cleanly while breaking every caller.
#[test]
fn an_id_flux_cannot_declare_is_refused() {
    let mut connector = zendesk_comment_list();
    connector.operations[0].id = "zendesk.ticket.comment.list".to_string();

    let err = emit_operation(&connector, &connector.operations[0])
        .expect_err("a dotted op declaration name does not parse in flux-lang");
    assert!(
        err.to_string().contains("C-23"),
        "the refusal must point at the story that owns naming, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// C-9 — bodies, headers, and the response
// ---------------------------------------------------------------------------

/// A synthetic operation carrying a **caller-supplied header** parameter. The launch inventory has
/// none — every provider's headers are either the fixed `content-type` or the credential the host
/// injects — so this fixture is constructed rather than cited, and says so.
fn headered_operation() -> Connector {
    connector(
        "vendor",
        "https://api.example.com",
        Operation {
            id: "vendor-thing-create".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Post,
            path: "/v1/things".to_string(),
            description: "Create a thing.".to_string(),
            risk: Risk::High,
            idempotency: Idempotency::NonIdempotent,
            repeatable_because: None,
            auth: None,
            params: ParamSet {
                path: Vec::new(),
                query: Vec::new(),
                header: vec![param(
                    "Idempotency-Key",
                    "A caller-chosen key that makes a retry safe.",
                    true,
                    json!({"type": "string"}),
                )],
                body: vec![param(
                    "label",
                    "What to call the thing.",
                    true,
                    json!({"type": "string"}),
                )],
                body_schema: None,
                ..ParamSet::default()
            },
            response_schema: None,
            quirks: Default::default(),
        },
    )
}

/// The body is assembled as a Flux record and handed to `http.request` **by symbol**. That is not a
/// style choice: `http.request` reads its `body` argument with `Value::as_str`
/// (`../flux/crates/flux-web/src/http.rs:183-186`), so an inline record would be silently dropped,
/// while a bound record is stored as canonical JSON text and arrives intact.
#[test]
fn a_post_assembles_its_json_body_from_the_ir_body_params() {
    let emitted = emit_only_operation(&freshdesk_note_add());
    assert!(
        emitted.contains(r#"payload = { body, incoming, notify_emails, private }"#),
        "the body params must assemble into one record:\n{emitted}"
    );
    assert!(
        emitted.contains("body: payload"),
        "the record must reach the request by symbol, not inline:\n{emitted}"
    );
    assert!(
        emitted.contains(r#"method: "POST""#),
        "the IR's method must travel:\n{emitted}"
    );
}

/// A request that carries a body carries the media type that describes it, and the static value is
/// bound rather than inlined — an all-literal record is not a fixed point of flux's formatter.
#[test]
fn a_body_bearing_request_declares_its_content_type() {
    let emitted = emit_only_operation(&freshdesk_note_add());
    assert!(
        emitted.contains(r#"content_type = "application/json""#),
        "the static media type must be bound:\n{emitted}"
    );
    assert!(
        emitted.contains(r#"headers: { "content-type": content_type }"#),
        "the media type must travel as a request header:\n{emitted}"
    );
}

/// A GET has nothing to describe, so it grows neither a payload nor a `content-type`.
#[test]
fn a_request_without_a_body_carries_no_content_type() {
    let emitted = emit_only_operation(&zendesk_comment_list());
    assert!(
        !emitted.contains("content_type") && !emitted.contains("payload"),
        "a bodiless GET must stay bare:\n{emitted}"
    );
}

/// A caller-supplied header travels under the vendor's own header name, valued by the op's symbol —
/// the same wire-name/symbol-name split query parameters already use.
#[test]
fn parameterized_headers_travel_under_their_wire_name() {
    let emitted = emit_only_operation(&headered_operation());
    assert!(
        emitted.contains("Idempotency_Key: String"),
        "the declared param must be a spellable Flux symbol:\n{emitted}"
    );
    assert!(
        emitted.contains(r#""Idempotency-Key": Idempotency_Key"#),
        "the header must keep the vendor's spelling:\n{emitted}"
    );
    assert!(
        emitted.contains(r#""content-type": content_type"#),
        "a static and a parameterized header must coexist:\n{emitted}"
    );
}

/// C-8 ended every op in a bare `do http.request`, which discards the statement result. The response
/// is now bound and returned explicitly, so what the op yields is stated rather than inherited from
/// the runtime's fall-through.
#[test]
fn the_response_is_bound_and_returned_explicitly() {
    for connector in [zendesk_test(), freshdesk_note_add(), zendesk_ticket_show()] {
        let emitted = emit_only_operation(&connector);
        assert!(
            emitted.contains("response = http.request("),
            "the response must be bound:\n{emitted}"
        );
        assert!(
            emitted.trim_end().ends_with("return response"),
            "the op must end by returning it:\n{emitted}"
        );
        assert!(
            !emitted.contains("do http.request"),
            "no statement may discard the response:\n{emitted}"
        );
    }
}

/// **The safety property of this story's response half.** `http.request` treats a non-2xx as data —
/// a 404 comes back with its status and the op *succeeds*
/// (`../flux/crates/flux-web/src/http.rs:219-225`). Generated Flux must not undo that by asserting
/// on the response, because a 404 that a caller can read and branch on is strictly more useful than
/// an aborted flow.
#[test]
fn a_non_2xx_response_is_data_not_an_op_failure() {
    for connector in [zendesk_ticket_show(), freshdesk_note_add()] {
        let emitted = emit_only_operation(&connector);
        for aborting in ["assert", "verify", "confirm"] {
            assert!(
                !emitted.contains(aborting),
                "`{aborting}` would turn a non-2xx result into a failure:\n{emitted}"
            );
        }
    }
}

/// The vendor's error envelope reaches the caller through the op's **tool contract**: the whole
/// response — status line, headers, body — is what the op returns, and the description says where
/// inside it the vendor hid the message and the code.
#[test]
fn a_declared_error_envelope_is_surfaced_on_the_op_contract() {
    let emitted = emit_only_operation(&zendesk_ticket_show());
    let module = flux_lang::program::Module::parse_str(&emitted).expect("emitted Flux must parse");
    let program = module.program().expect("an `op` declaration is a program");
    let description = &program.ops[0].meta.description;

    assert!(
        description.starts_with("Show one ticket."),
        "the vendor's own description must come first: {description}"
    );
    assert!(
        description.contains("/description") && description.contains("/error"),
        "both envelope pointers must be named: {description}"
    );
    assert!(
        description.contains("non-2xx"),
        "the contract must say a non-2xx is a result: {description}"
    );
}

/// An operation with no declared envelope says nothing about one — silence is not a claim.
#[test]
fn an_operation_without_an_envelope_keeps_its_description() {
    let emitted = emit_only_operation(&zendesk_comment_list());
    let module = flux_lang::program::Module::parse_str(&emitted).expect("emitted Flux must parse");
    let program = module.program().expect("an `op` declaration is a program");
    assert_eq!(
        program.ops[0].meta.description,
        "List one Zendesk ticket's comments."
    );
}

/// **The safety property of this story's metadata half.** flux's approval gate reads `risk` and
/// `idempotency`; a POST that inherited a GET's `low`/`idempotent` would be waved through. The
/// emitter refuses such a declaration rather than emitting it or silently correcting it — a silent
/// correction would hide the authoring bug that produced it.
#[test]
fn a_write_may_not_claim_a_reads_risk() {
    for method in [
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Patch,
        HttpMethod::Delete,
    ] {
        let mut connector = freshdesk_note_add();
        connector.operations[0].method = method;
        connector.operations[0].risk = Risk::Low;

        let err = emit_operation(&connector, &connector.operations[0])
            .expect_err("a write declared `low` must be refused");
        assert!(
            err.to_string().contains("low"),
            "the refusal must name the field, got: {err}"
        );
    }
}

/// POST and PATCH are not idempotent methods, so an op that claims they are would make a `retry`
/// around the call unsound — one charge becomes three.
#[test]
fn a_post_may_not_claim_to_be_idempotent() {
    for method in [HttpMethod::Post, HttpMethod::Patch] {
        let mut connector = freshdesk_note_add();
        connector.operations[0].method = method;
        connector.operations[0].idempotency = Idempotency::Idempotent;

        let err = emit_operation(&connector, &connector.operations[0])
            .expect_err("an idempotent POST/PATCH must be refused");
        assert!(
            err.to_string().contains("idempoten"),
            "the refusal must name the field, got: {err}"
        );
    }
}

/// `providers/zendesk.toml` pins `ticket.safe_update` to `true` with a JSON Schema `const`, because
/// the IR has no "always emitted, never caller-supplied" field. `const` already *means* exactly
/// that, so the emitter reads it: the field is sent, and it does not become a parameter a model has
/// to guess. Dropping `safe_update` turns every Zendesk write into a last-write-wins race, so this
/// is a safety property rather than a signature tidy-up.
#[test]
fn a_constant_body_field_is_sent_but_not_declared() {
    let emitted = emit_only_operation(&constant_body_operation());
    assert!(
        emitted.contains("safe_update = true"),
        "the constant must be bound:\n{emitted}"
    );
    let payload_line = emitted
        .lines()
        .find(|line| line.trim_start().starts_with("payload = "))
        .expect("the op binds a payload");
    assert!(
        payload_line.contains("safe_update"),
        "the constant must reach the payload:\n{emitted}"
    );
    let signature = emitted.lines().next().expect("a declaration line");
    assert!(
        !signature.contains("safe_update"),
        "a constant is not a caller parameter: {signature}"
    );
    assert!(
        signature.contains("body: String"),
        "a caller-supplied field still is: {signature}"
    );
}

/// `freshdesk-ticket-note-add` with a pinned constant field — the Zendesk `safe_update` shape,
/// exercised on a fixture whose body the emitter can actually spell.
fn constant_body_operation() -> Connector {
    let mut connector = freshdesk_note_add();
    connector.operations[0].params.body.push(param(
        "safe_update",
        "Constant `true`; not caller-supplied.",
        true,
        json!({"type": "boolean", "const": true}),
    ));
    connector
}

/// A body field whose *name* is dotted and which declares no `wire` is still refused: the dotted
/// spelling means either one field literally called `ticket.comment.body` or a path, and the two
/// produce different requests. Declaring `wire` is what decides it — see
/// [`golden_zendesk_ticket_comment_add`].
#[test]
fn a_dotted_body_name_without_a_wire_path_is_refused() {
    let mut connector = freshdesk_note_add();
    connector.operations[0].params.body = vec![param(
        "ticket.comment.body",
        "The comment text.",
        true,
        json!({"type": "string"}),
    )];

    let err = emit_operation(&connector, &connector.operations[0])
        .expect_err("a dotted body name with no `wire` is undecidable");
    assert!(
        err.to_string().contains("ticket.comment.body") && err.to_string().contains("wire"),
        "the refusal must name the field and the field that fixes it, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// C-29 — wire paths and wire aliases
// ---------------------------------------------------------------------------

/// `zendesk-ticket-comment-add` — **the nested-body fixture**, transcribed from
/// `providers/zendesk.toml` so the golden is the text a real build emits. Zendesk's wire body is
/// `{"ticket": {"updated_stamp": …, "safe_update": true, "comment": {"body": …, "public": …}}}`
/// (inventory §3.3.1). A flat body is not an error Zendesk reports — it answers 200 and applies
/// nothing — which is why this shape is pinned by a golden rather than by a `contains`.
fn zendesk_comment_add() -> Connector {
    connector(
        "zendesk",
        "https://example.zendesk.com",
        Operation {
            id: "zendesk-ticket-comment-add".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Put,
            path: "/api/v2/tickets/{ticket_id}.json".to_string(),
            description:
                "Add a comment to a ticket; the comment is an internal note unless public is \
                 explicitly true"
                    .to_string(),
            risk: Risk::Medium,
            // Optimistic concurrency: replaying the call after the ticket moved is rejected by
            // Zendesk rather than applied, so repeating is safe only under the caller's stamp.
            //
            // That sentence used to live only here. C-186 requires a mutating `conditional` to state
            // its condition in the IR, so it moves into the field below — where a host can read it —
            // and this fixture tracks `providers/zendesk.toml`, which made the same move.
            idempotency: Idempotency::Conditional,
            repeatable_because: Some(
                "the caller supplies `updated_stamp` and Zendesk applies the write only if the \
                 ticket has not moved since, so a stale replay is rejected rather than duplicated"
                    .to_string(),
            ),
            auth: None,
            params: ParamSet {
                path: vec![param(
                    "ticket_id",
                    "The ticket id",
                    true,
                    json!({"type": "integer", "format": "uint64", "minimum": 1}),
                )],
                body: vec![
                    wired(
                        "updated_stamp",
                        "ticket.updated_stamp",
                        "Optimistic-concurrency stamp",
                        true,
                        json!({"type": "string"}),
                    ),
                    wired(
                        "safe_update",
                        "ticket.safe_update",
                        "Constant `true` — not caller-supplied",
                        true,
                        json!({"type": "boolean", "const": true}),
                    ),
                    wired(
                        "body",
                        "ticket.comment.body",
                        "The comment text; must not be blank",
                        true,
                        json!({"type": "string", "minLength": 1}),
                    ),
                    wired(
                        "public",
                        "ticket.comment.public",
                        "False (an internal note) unless explicitly set true",
                        false,
                        json!({"type": "boolean", "default": false}),
                    ),
                ],
                ..ParamSet::default()
            },
            response_schema: None,
            quirks: Default::default(),
        },
    )
}

/// `babelforce-call-session-set` — the **free-form body** fixture.
/// `SetCallSessionVariablesRequest` is `{"type": "object"}` with no properties (inventory §6.5), so
/// there are no named fields to assemble: the caller supplies the whole body.
fn babelforce_session_set() -> Connector {
    connector(
        "babelforce",
        "https://services.babelforce.com",
        Operation {
            id: "babelforce-call-session-set".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Put,
            path: "/api/v2/calls/{id}/session/set".to_string(),
            description: "Set session variables on a live call.".to_string(),
            risk: Risk::Medium,
            idempotency: Idempotency::Idempotent,
            repeatable_because: None,
            auth: None,
            params: ParamSet {
                path: vec![param("id", "The call id", true, json!({"type": "string"}))],
                body_schema: Some(json!({
                    "type": "object",
                    "description": "The session variables to set, as a JSON object.",
                })),
                ..ParamSet::default()
            },
            response_schema: None,
            quirks: Default::default(),
        },
    )
}

/// **The golden this story exists for.** A real Zendesk write, with a real nested body.
#[test]
fn golden_zendesk_ticket_comment_add() {
    assert_golden(
        "zendesk-ticket-comment-add.flux",
        &emit_only_operation(&zendesk_comment_add()),
    );
}

#[test]
fn golden_babelforce_call_session_set() {
    assert_golden(
        "babelforce-call-session-set.flux",
        &emit_only_operation(&babelforce_session_set()),
    );
}

/// The op declares the **caller-facing** names and the payload nests under the **wire** paths.
/// Neither name is derivable from the other, which is why the IR carries both.
#[test]
fn a_body_field_travels_at_the_json_path_its_wire_names() {
    let emitted = emit_only_operation(&zendesk_comment_add());
    assert!(
        emitted.contains(
            "payload = { ticket: { comment: { body, public }, safe_update, updated_stamp } }"
        ),
        "the body must nest under its wire paths:\n{emitted}"
    );
    let signature = emitted.lines().next().expect("a declaration line");
    assert!(
        signature.contains("body: String") && signature.contains("public: Bool"),
        "the caller-facing names are what the op declares: {signature}"
    );
    assert!(
        !signature.contains("ticket.comment"),
        "a wire path is not a parameter name: {signature}"
    );
    assert!(
        !signature.contains("safe_update"),
        "a constant stays out of the signature even when it nests: {signature}"
    );
}

/// A free-form body is re-bound through `parse(…, as: "json")` rather than handed to
/// `http.request` directly. That is a correctness requirement, not a style: a composite op's
/// parameter is stored with `Value::from_json` (flux-lang `runtime.rs:313-331`), so a
/// caller-supplied record arrives as a `Value::Struct`; `http.request` reads its `body` argument
/// with `Value::as_str` (`../flux/crates/flux-web/src/http.rs:182-186`) and would send **no body at
/// all**. `parse` canonicalizes a record and validates a JSON string, storing text either way.
#[test]
fn a_free_form_body_is_canonicalized_before_it_is_sent() {
    let emitted = emit_only_operation(&babelforce_session_set());
    assert!(
        emitted.contains(r#"payload = parse(body, as: "json")"#),
        "the caller's body must be canonicalized to text:\n{emitted}"
    );
    assert!(
        emitted.contains("body: payload"),
        "the payload must reach the request by symbol:\n{emitted}"
    );
    assert!(
        !emitted.contains("http.request(body,") && !emitted.contains("body: body"),
        "passing the parameter straight through would send no body at all:\n{emitted}"
    );
    assert!(
        emitted.contains(r#"content_type = "application/json""#),
        "a free-form body still describes itself:\n{emitted}"
    );
    let signature = emitted.lines().next().expect("a declaration line");
    assert!(
        signature.contains("body: Any"),
        "the whole body is one declared parameter: {signature}"
    );
}

/// "The body is these fields" and "the body is this schema" are two answers to one question, and
/// nothing states how to merge them — so an operation that declares both is refused rather than
/// having one of the two silently dropped.
#[test]
fn a_body_that_is_both_a_schema_and_a_field_list_is_refused() {
    let mut connector = babelforce_session_set();
    connector.operations[0].params.body = vec![param(
        "label",
        "A named field alongside a free-form body.",
        true,
        json!({"type": "string"}),
    )];

    let err = emit_operation(&connector, &connector.operations[0])
        .expect_err("a body cannot be assembled and supplied whole at once");
    assert!(
        err.to_string().contains("body_schema"),
        "the refusal must name both declarations, got: {err}"
    );
}

/// A query parameter's `wire` is a plain alias: Freshdesk's requester filter is `req_id` to a
/// caller and `requester_id` on the wire (inventory §4.2 op 2). The op declares the name a model
/// reads; the query string carries the vendor's.
#[test]
fn a_query_alias_travels_under_its_wire_name() {
    let mut connector = zendesk_ticket_search();
    connector.operations[0].params.query = vec![
        wired(
            "q",
            "query",
            "A Zendesk search expression.",
            true,
            json!({"type": "string"}),
        ),
        wired(
            "req_id",
            "requester_id",
            "Filter by requester.",
            false,
            json!({"type": "string"}),
        ),
    ];

    let emitted = emit_only_operation(&connector);
    assert!(
        emitted.contains(r#"url = fmt("{base}/api/v2/search.json?query={q}")"#),
        "a required alias must reach the vendor under its wire name:\n{emitted}"
    );
    assert!(
        emitted.contains(r#"fmt("{url}{sep}requester_id={req_id}")"#),
        "an optional alias must too:\n{emitted}"
    );
    let signature = emitted.lines().next().expect("a declaration line");
    assert!(
        signature.contains("q: String") && signature.contains("req_id: String"),
        "the op declares the caller-facing names: {signature}"
    );
}

/// The same field on a header, where the wire name is also what `http.request` validates as an HTTP
/// token.
#[test]
fn a_header_alias_travels_under_its_wire_name() {
    let mut connector = headered_operation();
    connector.operations[0].params.header = vec![wired(
        "idempotency_key",
        "Idempotency-Key",
        "A caller-chosen key that makes a retry safe.",
        true,
        json!({"type": "string"}),
    )];

    let emitted = emit_only_operation(&connector);
    assert!(
        emitted.contains(r#""Idempotency-Key": idempotency_key"#),
        "the header must keep the vendor's spelling:\n{emitted}"
    );
    assert!(
        emitted.contains("idempotency_key: String"),
        "the op declares the caller-facing name:\n{emitted}"
    );
}
