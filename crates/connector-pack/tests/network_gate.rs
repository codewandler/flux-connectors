//! **The mirrored network gate**, over the whole shipped catalogue.
//!
//! `Tool::execute` delegates to flux's own `http.request` tool by calling its `execute` directly.
//! That call **bypasses `Executor::dispatch`**, so `http.request`'s own `permission_subjects` and
//! `intents` are never consulted for the inner call. Both have default trait implementations that
//! return empty, so a projected Tool that omits them compiles, registers, executes, reaches the
//! vendor — and the host's network policy is never asked. Every test still passes.
//!
//! This file is the defence. It asserts the gate over **every** shipped operation rather than a
//! sampled one, because a subject that is right for `zendesk-ticket-show` and empty for
//! `google-calendar-calendar-get` is a hole in exactly the connector nobody checked.

use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{Credentials, Egress, MemoryStore};
use flux_runtime::{Tool, ToolRegistry};
use serde_json::{json, Map, Value};

/// A stand-in for flux's `http.request`. Nothing here reaches it: the gate is what a host consults
/// *before* dispatch, so it is asserted without a transport ever being used.
fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        },
        |params| async move { Ok(params) },
    ))
}

/// A bound credential port over an **empty** store (C-116).
///
/// The pack requires one, so the gate cannot be asserted without binding it. It holds nothing on
/// purpose: the gate is what a host consults *before* dispatch, and every subject asserted below is
/// the **unauthenticated** URL — a credential resolving here would be the one way a query-placed
/// secret could end up in a permission subject, which is precisely what this file must not let
/// happen quietly.
fn credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), "t-network-gate").expect("a valid tenant id")
}

/// Every provider the catalogue ships, in its own stable order.
fn every_provider() -> Vec<&'static str> {
    catalog::providers()
        .iter()
        .map(|provider| provider.id)
        .collect()
}

/// A registry holding the whole catalogue.
fn whole_catalogue() -> ToolRegistry {
    let providers = every_provider();
    let mut registry = ToolRegistry::new();
    connector_pack::pack(&providers, http(), credentials())(&mut registry)
        .expect("the shipped catalogue installs");
    registry
}

/// A plausible value for every parameter the tool declares, taken from its own input schema.
///
/// Driven by the schema rather than by a hand-written table so it covers all 97 operations,
/// including ones added after this test was written.
fn params_for(tool: &dyn Tool) -> Value {
    let spec = tool.spec();
    let mut params = Map::new();
    let Some(properties) = spec
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    else {
        return Value::Object(params);
    };
    for (name, schema) in properties {
        let value = match schema.get("type").and_then(Value::as_str) {
            Some("number") | Some("integer") => Value::from(1),
            Some("boolean") => Value::Bool(true),
            Some("array") => Value::Array(Vec::new()),
            Some("object") => Value::Object(Map::new()),
            Some(_) => Value::String(format!("a-{name}")),
            // An untyped schema is a free-form body (`Any`), which reaches the vendor through
            // `parse(…, as: "json")`. Supplying a bare string here would make the request
            // unbuildable and silently move this test onto the declared-host fallback — the one
            // path it is not trying to exercise.
            None => Value::Object(Map::new()),
        };
        params.insert(name.clone(), value);
    }
    Value::Object(params)
}

/// **The Acceptance test.** Every shipped operation declares the host it reaches.
///
/// Non-empty is the load-bearing half: `Vec::new()` is what the trait hands an implementor for
/// free, and it is indistinguishable from a considered answer at every other layer.
#[test]
fn every_tool_declares_the_host_it_reaches() {
    let registry = whole_catalogue();
    let mut checked = 0usize;

    for entry in catalog::operations() {
        let dotted = connector_pack::dotted_name(entry.id)
            .unwrap_or_else(|error| panic!("`{}` has no dotted tool name: {error}", entry.id));
        let tool = registry
            .get(&dotted)
            .unwrap_or_else(|| panic!("`{}` is not registered as `{dotted}`", entry.id));

        let subjects = tool.permission_subjects(&params_for(tool.as_ref()));
        assert!(
            !subjects.is_empty(),
            "`{}` declares no permission subject, so delegating to `http.request` would reach \
             {:?} with the host's network policy never consulted",
            entry.id,
            entry.hosts
        );

        // The declared data, not a re-parse of the URL template: the manifest's `http_hosts` is
        // what an operator's egress policy was written against.
        assert!(
            !entry.hosts.is_empty(),
            "`{}` declares no host, so no subject could name one",
            entry.id
        );
        for host in entry.hosts {
            assert!(
                subjects.iter().any(|subject| subject.contains(host)),
                "`{}` reaches `{host}` but its subjects are {subjects:?}",
                entry.id
            );
        }
        checked += 1;
    }

    assert!(
        checked > 0,
        "an empty catalogue would pass every assertion above"
    );
}

/// The other half of the gate: `http.request` raises `NetworkFetch` at `:126`, and the inner call
/// never consults it, so the projected Tool must raise it itself.
#[test]
fn every_tool_raises_a_network_fetch_intent() {
    let registry = whole_catalogue();

    for entry in catalog::operations() {
        let dotted = connector_pack::dotted_name(entry.id).expect("a dotted tool name");
        let tool = registry.get(&dotted).expect("the operation is registered");
        let intents = tool.intents(&params_for(tool.as_ref()));

        assert!(
            intents
                .intents
                .iter()
                .any(|intent| matches!(intent.behavior, flux_spec::IntentBehavior::NetworkFetch)),
            "`{}` raises {:?}, which does not include the NetworkFetch `http.request` would have \
             raised had the call gone through `Executor::dispatch`",
            entry.id,
            intents.intents
        );
    }
}

/// A subject must survive params a caller got wrong. `permission_subjects` cannot fail — it returns
/// a `Vec` — so an unbuildable request has to fall back to the declared host rather than to
/// silence, or the one call most likely to be malformed is also the one nobody gates.
#[test]
fn a_tool_still_declares_its_host_when_the_request_cannot_be_built() {
    let registry = whole_catalogue();
    let entry = catalog::operation(OperationKey::id("zendesk-ticket-show"))
        .expect("the shipped catalogue carries zendesk-ticket-show");
    let tool = registry
        .get("zendesk.ticket.show")
        .expect("the operation is registered");

    let subjects = tool.permission_subjects(&serde_json::json!({}));
    assert!(
        !subjects.is_empty(),
        "a call with no parameters at all must still declare where it would go"
    );
    for host in entry.hosts {
        assert!(
            subjects.iter().any(|subject| subject.contains(host)),
            "the fallback must name the declared host, got {subjects:?}"
        );
    }
}
