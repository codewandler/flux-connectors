//! The C-8 emitter contract: one IR [`Operation`] becomes one formatted Flux `op` declaration.
//!
//! Three properties are pinned here, and they are the ones every later codegen story inherits:
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
    Connector, HttpMethod, Idempotency, Operation, Param, ParamSet, Provenance, Risk,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// Fixtures — real operations, cited to the inventory
// ---------------------------------------------------------------------------

fn param(name: &str, description: &str, required: bool, schema: serde_json::Value) -> Param {
    Param {
        name: name.to_string(),
        description: description.to_string(),
        required,
        schema,
    }
}

fn connector(id: &str, base_url: &str, operation: Operation) -> Connector {
    Connector {
        id: id.to_string(),
        vendor: String::new(),
        base_url: base_url.to_string(),
        description: String::new(),
        auth: Vec::new(),
        default_auth: Vec::new(),
        operations: vec![operation],
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
            method: HttpMethod::Get,
            path: "/api/v2/tickets/{ticket_id}/comments.json".to_string(),
            description: "List one Zendesk ticket's comments.".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
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
            method: HttpMethod::Get,
            path: "/api/v2/search.json".to_string(),
            description: "Search Zendesk tickets with Zendesk search syntax.".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
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
            method: HttpMethod::Get,
            path: "/api/v2/calls/reporting".to_string(),
            description: "List and filter calls, in the reporting view.".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
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
            },
            response_schema: None,
            quirks: Default::default(),
        },
    )
}

/// `zendesk-test` — the floor of the shape: no parameters at all, so no `$sep` and no guards.
/// Inventory §3.2 op 1 (`../flux/plugins/zendesk/src/main.rs:19`).
fn zendesk_test() -> Connector {
    connector(
        "zendesk",
        "https://example.zendesk.com",
        Operation {
            id: "zendesk-test".to_string(),
            method: HttpMethod::Get,
            path: "/api/v2/users/me.json".to_string(),
            description: "Verify Zendesk credentials by fetching the authenticated user."
                .to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
            auth: None,
            params: ParamSet::default(),
            response_schema: None,
            quirks: Default::default(),
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
        emitted.contains(r#"$url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")"#),
        "expected the path template interpolated into $url in:\n{emitted}"
    );
}

/// Query parameters assemble onto the request: a required one is always present, an optional one is
/// guarded so an unsupplied filter never reaches the vendor as an empty `key=`.
#[test]
fn query_parameters_assemble_into_the_request() {
    let emitted = emit_only_operation(&zendesk_ticket_search());
    assert!(
        emitted.contains(r#"$url = fmt("{base}/api/v2/search.json?query={query}")"#),
        "a required query parameter belongs in the base URL:\n{emitted}"
    );
    assert!(
        emitted.contains("when $page\n") && emitted.contains(r#"fmt("{url}{sep}page={page}")"#),
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
        !emitted.contains("$time.start"),
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

/// Request bodies and caller-supplied headers are C-9 and C-10. The emitter refuses them loudly
/// rather than emitting a request that quietly drops them.
#[test]
fn out_of_slice_operations_are_refused_rather_than_half_emitted() {
    let mut connector = zendesk_comment_list();
    connector.operations[0].params.body = vec![param(
        "comment",
        "The comment body.",
        true,
        json!({"type": "string"}),
    )];

    let err = emit_operation(&connector, &connector.operations[0])
        .expect_err("a body parameter is outside the C-8 slice");
    assert!(
        err.to_string().contains("body"),
        "the refusal must name what it cannot emit, got: {err}"
    );
}
