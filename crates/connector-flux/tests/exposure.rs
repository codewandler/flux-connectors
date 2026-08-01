//! **An operation can be callable without being an LLM tool.**
//!
//! The emitter hard-coded `expose: true` (`src/op.rs`, `src/graph.rs`), which fused two claims that
//! are not the same claim: that an operation **exists and can be called**, and that it **reaches a
//! model as a tool**. Fusing them means a full-coverage connector is 397 tools, which is not a
//! catalogue but a denial of service against the model's context — and it is the whole reason
//! `docs/designs/provider-operation-inventory.md` §5.2 curated 9 of babelforce's 163 rather than
//! shipping them all.
//!
//! `expose` splits them. It **defaults to `true`**, so this is a widening and not a loosening: every
//! operation that says nothing emits exactly the module it emitted before, and the new state is
//! strictly more restrictive than anything an author could already express.
//!
//! What this file pins is the emitter half. That an unexposed operation stays **catalogued and
//! callable** — manifest, catalogue, and a built request — is `connector-pack`'s
//! `tests/exposure.rs` and `connector-cli`'s `tests/exposure_artifacts.rs`; the point of the split
//! is that withholding the tool withholds *only* the tool.

use connector_flux::{emit_graph, emit_operation};
use connector_spec::provider;

/// The smallest provider file that declares one operation and one flow over it, each saying whatever
/// the caller wants about `expose`.
///
/// Both halves go through the **loader** rather than through in-memory `Operation`/`Graph` literals,
/// because `expose` has to be a thing a provider file can *say* before it can be a thing the emitter
/// reads — and a test built from struct literals would pass over a loader that silently dropped the
/// key.
fn provider_toml(operation_expose: Option<bool>, graph_expose: Option<bool>) -> String {
    let declaration = |expose: Option<bool>| {
        expose
            .map(|value| format!("expose = {value}\n"))
            .unwrap_or_default()
    };
    format!(
        r#"
id = "acme"
vendor = "Acme"
description = "A connector that exists to exercise one guard"
base_url = "https://api.acme.test"
api_version = "v1"

[[operations]]
id = "acme-thing-list"
method = "GET"
path = "/v1/things"
description = "List the things."
risk = "low"
idempotency = "idempotent"
{}
[[graphs]]
name = "acme-thing-flow"
description = "Read the things."
{}
[[graphs.nodes]]
id = "list"
kind = {{ operation = {{ operation = "acme-thing-list" }} }}
"#,
        declaration(operation_expose),
        declaration(graph_expose),
    )
}

/// Load a one-operation fixture and emit that operation, or return the refusal as a string.
fn emit(expose: Option<bool>) -> Result<String, String> {
    let toml = provider_toml(expose, None);
    let connector = provider::load("acme", &toml)
        .map_err(|error| error.to_string())?
        .connector;
    let operation = connector
        .operations
        .first()
        .expect("the fixture declares one operation")
        .clone();
    emit_operation(&connector, &operation).map_err(|error| error.to_string())
}

/// Load the same fixture and emit its flow instead.
fn emit_flow(expose: Option<bool>) -> Result<String, String> {
    let toml = provider_toml(None, expose);
    let connector = provider::load("acme", &toml)
        .map_err(|error| error.to_string())?
        .connector;
    let graph = connector
        .graphs
        .first()
        .expect("the fixture declares one flow")
        .clone();
    emit_graph(&connector, &graph).map_err(|error| error.to_string())
}

/// **C-11's gate, applied to one emitted module**: it parses, it is already canonical, and it loads
/// as exactly one composite op — whose `expose` is the value the author declared.
///
/// The exposure assertion rides *inside* the gate deliberately. An emitter that wrote `expose false`
/// into text flux cannot parse, or that flux's formatter would rewrite, would have produced a
/// plausible artifact rather than a valid one, and asserting the substring alone would not notice.
fn assert_gate(emitted: &str, id: &str, expected_expose: bool) {
    let parsed = flux_lang::parser::parse_cst(emitted);
    assert!(
        parsed.errors.is_empty(),
        "`{id}` emits Flux that does not parse: {:?}\n{emitted}",
        parsed.errors
    );
    assert_eq!(
        flux_lang::format_cst::format_module(&parsed).as_deref(),
        Some(emitted),
        "the flux formatter would rewrite `{id}`"
    );

    let module = flux_lang::program::Module::parse_str(emitted)
        .unwrap_or_else(|error| panic!("`{id}` does not load: {error}"));
    let program = module
        .program()
        .unwrap_or_else(|| panic!("`{id}` is not a program"));
    assert_eq!(program.ops.len(), 1, "`{id}` must load as one composite op");
    assert_eq!(
        program.ops[0].meta.expose, expected_expose,
        "`{id}` must load with `expose {expected_expose}`:\n{emitted}"
    );
}

/// **The story's failing-first test.** An operation declaring `expose = false` emits a module that
/// says so, and that module still passes C-11's gate.
///
/// Before the field existed this failed at the *loader*: `Operation` is `deny_unknown_fields`, so
/// `expose = false` was not a thing a provider file could say at all.
#[test]
fn an_unexposed_operation_emits_a_module_declaring_expose_false() {
    let emitted = emit(Some(false)).expect("a provider may declare an operation unexposed");

    assert!(
        emitted.contains("expose false"),
        "an operation declaring `expose = false` must emit `expose false` — the whole point is that \
         the module states the exposure positively rather than leaving a reader to infer it from an \
         absence:\n{emitted}"
    );
    assert_gate(&emitted, "acme-thing-list", false);
}

/// The converse, so the test above cannot pass by the emitter simply never exposing anything.
#[test]
fn an_operation_declaring_expose_true_still_emits_expose_true() {
    let emitted = emit(Some(true)).expect("a provider may declare an operation exposed");

    assert!(emitted.contains("expose true"), "{emitted}");
    assert_gate(&emitted, "acme-thing-list", true);
}

/// **Silence is exposure**, which is what keeps every shipped artifact exactly where it was.
///
/// Asserted as byte equality against the `expose = true` rendering rather than as a substring: a
/// default that produced the right `expose` line and a different module anywhere else would still
/// have moved 557 artifacts.
#[test]
fn an_operation_silent_on_exposure_emits_exactly_what_expose_true_emits() {
    let silent = emit(None).expect("an operation may say nothing about exposure");
    let explicit = emit(Some(true)).expect("a provider may declare an operation exposed");

    assert_eq!(
        silent, explicit,
        "an operation saying nothing about `expose` must emit exactly what `expose = true` emits, \
         or landing this field rewrites every module in the repository"
    );
    assert_gate(&silent, "acme-thing-list", true);
}

// ---------------------------------------------------------------------------
// The graph half
// ---------------------------------------------------------------------------

/// `graph.rs` carried the same literal `true` as `op.rs`, so a flow could not be unexposed either.
///
/// A flow is where the distinction earns the most: a curated flow over uncurated operations is
/// exactly the shape C-413 exists to make expressible, and the reverse — an unexposed flow — is what
/// lets one exist as a callable building block without spending a tool slot.
///
/// A graph's `expose` is **authored, not derived**, which is the one place it parts company with the
/// `risk` and `idempotency` beside it. Those are derived from the operations the flow calls because a
/// flow that deletes must not inherit the `low` of the reads it also makes. Exposure has no such
/// floor to respect: a curated flow over uncurated operations is the whole point, so deriving it
/// from the called set would make the intended shape inexpressible.
#[test]
fn an_unexposed_graph_emits_a_module_declaring_expose_false() {
    let emitted = emit_flow(Some(false)).expect("a provider may declare a flow unexposed");
    assert!(emitted.contains("expose false"), "{emitted}");
    assert_gate(&emitted, "acme-thing-flow", false);

    let exposed = emit_flow(Some(true)).expect("a provider may declare a flow exposed");
    assert!(exposed.contains("expose true"), "{exposed}");
    assert_gate(&exposed, "acme-thing-flow", true);
}

/// A flow silent on exposure emits exactly what an explicitly-exposed one emits, for the same reason
/// the operation case does: silence must not move a byte.
#[test]
fn a_graph_silent_on_exposure_emits_exactly_what_expose_true_emits() {
    let silent = emit_flow(None).expect("a flow may say nothing about exposure");
    let explicit = emit_flow(Some(true)).expect("a provider may declare a flow exposed");

    assert_eq!(silent, explicit);
    assert_gate(&silent, "acme-thing-flow", true);
}
