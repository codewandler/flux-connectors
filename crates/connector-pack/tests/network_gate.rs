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

use std::sync::{Arc, OnceLock};

use catalog::OperationKey;
use connector_pack::{Configuration, Credentials, Egress, MemoryConfig, MemoryStore, Operation};
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
    Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant id")
}

/// The tenant both ports answer for.
const TENANT: &str = "t-network-gate";

/// The value this file binds for every endpoint variable, whatever it is called.
///
/// One spelling rather than a per-variable table, because what is asserted below is *that the
/// subject carries the configured value* — not which value. `a-subdomain.zendesk.com` is as good a
/// resolvable host for that as `acme.zendesk.com`, and it makes the assertion messages say which
/// variable was involved.
fn value_for(entry: &catalog::Operation, variable: &str) -> String {
    let binding = format!("endpoint.{variable}");
    let format = catalog::provider(catalog::ProviderKey::id(entry.provider))
        .and_then(|provider| {
            provider
                .config
                .iter()
                .find(|field| field.service == entry.service && field.binds == binding)
        })
        .map(|field| field.format);
    match format {
        Some("origin") => "https://self-managed.example:8443".to_owned(),
        _ => format!("a-{variable}"),
    }
}

/// A bound configuration port carrying a value for **every** endpoint variable the shipped
/// catalogue declares (C-193).
///
/// Discovered from the catalogue rather than listed, so a templated connector shipped after this
/// file was written is covered on the day it lands — which matters more here than anywhere else,
/// since this file's whole claim is that it asserts the gate over *every* operation rather than a
/// sampled one.
fn configuration() -> Configuration {
    static CONFIGURATION: OnceLock<Configuration> = OnceLock::new();

    CONFIGURATION.get_or_init(build_configuration).clone()
}

/// Build the immutable configuration snapshot shared by this test binary.
///
/// Discovering the fields is deliberately catalogue-wide, but doing that from every call to
/// [`tool_for`] made each assertion project the whole catalogue once per operation. The production
/// port remains explicitly host-bound; this cache exists only inside the test process.
fn build_configuration() -> Configuration {
    let mut values = MemoryConfig::new();
    for entry in catalog::operations() {
        for variable in probe(entry).endpoint_variables() {
            // Under the entry's own service (C-197): the same variable name in two services of one
            // connector is two values, so binding it once for the connector would leave every
            // operation of the second service unconfigured and gated against a templated host.
            let value = value_for(entry, variable);
            values = if value.starts_with("https://") {
                values.with_approved_endpoint(
                    TENANT,
                    entry.provider,
                    entry.service,
                    variable,
                    &value,
                )
            } else {
                values.with_endpoint(TENANT, entry.provider, entry.service, variable, &value)
            };
        }
    }
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

/// One entry projected against an empty configuration, purely to ask it which variables it names.
/// Projection reads no values, so this cannot fail for want of one.
fn probe(entry: &'static catalog::Operation) -> Operation {
    let empty = Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a tenant");
    Operation::project(entry, http(), credentials(), empty)
        .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id))
}

/// A declared host with this file's configuration filled in.
///
/// `entry.hosts` is the manifest's `http_hosts`, and for six connectors it is a **template**:
/// `{subdomain}.zendesk.com`. The host a request actually reaches is `a-subdomain.zendesk.com`, and
/// that is what a subject has to name — see
/// [`a_templated_host_is_never_declared_as_a_permission_subject`].
fn resolved_host(entry: &'static catalog::Operation, host: &str) -> String {
    let mut out = host.to_owned();
    for variable in probe(entry).endpoint_variables() {
        out = out.replace(&format!("{{{variable}}}"), &value_for(entry, variable));
    }
    out
}

/// Every provider the catalogue ships, in its own stable order.
fn every_provider() -> Vec<&'static str> {
    catalog::providers()
        .iter()
        .map(|provider| provider.id)
        .collect()
}

/// A registry holding the whole catalogue **as a model sees it** — the exposed operations only.
///
/// Kept for the one assertion that is about what a host advertises. Every assertion about what a
/// *call* does goes through [`tool_for`] instead; see its docs for why the difference is the whole
/// point of this file.
fn whole_catalogue() -> ToolRegistry {
    let providers = every_provider();
    let mut registry = ToolRegistry::new();
    connector_pack::pack(&providers, http(), credentials(), configuration())(&mut registry)
        .expect("the shipped catalogue installs");
    registry
}

/// **The projected tool for one catalogued operation, exposed or not** — C-413, C-417.
///
/// This used to read out of [`whole_catalogue`], and that was correct for exactly as long as every
/// shipped operation was exposed. It no longer is: babelforce publishes 391 operations of which
/// nine reach a model, so `pack` withholds 382 of them from the registry — and reading the gate out
/// of the registry would have quietly stopped asserting it for **97% of the catalogue**, while the
/// file's own header claims it covers "every shipped operation rather than a sampled one".
///
/// `resolve` is the seam that does not filter, and it is the right one here on the merits rather
/// than as a repair: unexposed withholds the *tool*, never the *call*, so an unexposed operation
/// still reaches the vendor through `Executor::dispatch` and still bypasses `http.request`'s own
/// `permission_subjects`. An operation a model cannot see but a caller can run is exactly the one
/// whose network gate nobody would think to check.
fn tool_for(entry: &'static catalog::Operation) -> Arc<dyn Tool> {
    connector_pack::resolve(entry, http(), credentials(), configuration())
        .unwrap_or_else(|error| panic!("`{}` does not resolve: {error}", entry.id))
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
    // The dotted name every exposed operation registers under, asserted once here so the switch to
    // `resolve` does not quietly stop covering registration itself.
    let registry = whole_catalogue();
    for entry in catalog::operations() {
        let dotted = connector_pack::dotted_name(entry.id)
            .unwrap_or_else(|error| panic!("`{}` has no dotted tool name: {error}", entry.id));
        if connector_pack::is_exposed(entry)
            .unwrap_or_else(|error| panic!("`{}` does not state its exposure: {error}", entry.id))
        {
            assert!(
                registry.get(&dotted).is_some(),
                "`{}` is exposed and is not registered as `{dotted}`",
                entry.id
            );
        }
    }

    let mut checked = 0usize;

    for entry in catalog::operations() {
        let tool = tool_for(entry);

        let subjects = tool.permission_subjects(&params_for(tool.as_ref()));
        assert!(
            !subjects.is_empty(),
            "`{}` declares no permission subject, so delegating to `http.request` would reach \
             {:?} with the host's network policy never consulted",
            entry.id,
            entry.hosts
        );

        // The declared data, not a re-parse of the URL template: the manifest's `http_hosts` is
        // what an operator's egress policy was written against — **once its `{placeholder}`s carry
        // this tenant's values**, which is the part C-193 added.
        assert!(
            !entry.hosts.is_empty(),
            "`{}` declares no host, so no subject could name one",
            entry.id
        );
        for host in entry.hosts {
            let host = resolved_host(entry, host);
            assert!(
                subjects.iter().any(|subject| subject.contains(&host)),
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

/// The other half of the gate: the inner `http.request` never gets to contribute its intent through
/// `Executor::dispatch`, so the projected Tool must raise the direction-appropriate network intent
/// itself. Reads fetch; writes connect to a mutation target.
#[test]
fn every_tool_raises_its_authored_direction_as_a_network_intent() {
    for entry in catalog::operations() {
        let tool = tool_for(entry);
        let intents = tool.intents(&params_for(tool.as_ref()));
        let expected = match entry.direction {
            catalog::OperationDirection::Read => flux_spec::IntentBehavior::NetworkFetch,
            catalog::OperationDirection::Write => flux_spec::IntentBehavior::NetworkConnect,
        };

        assert!(
            intents
                .intents
                .iter()
                .any(|intent| intent.behavior == expected),
            "`{}` raises {:?}, which does not include its authored {:?} direction as a network \
             intent",
            entry.id,
            intents.intents,
            entry.direction
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
        let host = resolved_host(entry, host);
        assert!(
            subjects.iter().any(|subject| subject.contains(&host)),
            "the fallback must name the declared host, got {subjects:?}"
        );
    }
    // The fallback is the path most likely to be forgotten — it fires exactly when the request
    // could not be built — so it gets the placeholder assertion of its own. Substituting on the
    // successful path only would leave a gate that is correct until a caller gets a parameter
    // wrong, and then silently is not.
    for subject in &subjects {
        assert!(
            !subject.contains('{'),
            "the fallback declared `{subject}`, which no egress allow-list can match"
        );
    }
}

/// **The second half of C-193, over the whole catalogue.**
///
/// `Tool::execute` calls `http.request`'s `execute` directly, so this is the *only* place a host's
/// egress allow-list is consulted for the inner call. A subject of
/// `https://{subdomain}.zendesk.com/api/v2/tickets/1` asks that allow-list to match a string no
/// host ever resolves to, and there are only two ways that ends: the call is refused for a reason
/// that names nothing an operator can fix, or the operator widens the rule to a wildcard until it
/// passes — which is the gate being removed rather than satisfied.
///
/// Asserted over every shipped operation rather than over Zendesk, because the six templated
/// connectors are the ones nobody would think to check twice.
#[test]
fn a_templated_host_is_never_declared_as_a_permission_subject() {
    let mut templated = 0usize;

    for entry in catalog::operations() {
        let tool = tool_for(entry);
        if entry.hosts.iter().any(|host| host.contains('{')) {
            templated += 1;
        }

        for subject in tool.permission_subjects(&params_for(tool.as_ref())) {
            assert!(
                !subject.contains('{'),
                "`{}` declares the permission subject `{subject}`, which an egress allow-list \
                 cannot match — the request would reach a host the gate was never shown",
                entry.id
            );
        }
    }

    // The control. Without it this test passes trivially on a catalogue that happens to carry no
    // templated connector at all, which is precisely the state it is guarding against regressing
    // *from*.
    assert!(
        templated > 0,
        "no shipped connector declares a templated host, so this test asserted nothing"
    );
}
