//! The pack, applied to the **real** catalogue.
//!
//! The unit tests beside the projection prove one operation converts correctly. This file proves
//! the whole shipped catalogue does — every operation, every provider, into one registry — because
//! a projection that is right for `zendesk-ticket-show` and wrong for
//! `google-calendar-calendar-get` is a projection that ships broken.

use std::sync::Arc;

use catalog::{OperationKey, ProviderKey};
use connector_pack::{Configuration, Credentials, Egress, MemoryConfig, MemoryStore};
use flux_runtime::ToolRegistry;

/// A stand-in for flux's `http.request`, which every projected operation delegates its egress to.
///
/// A host supplies `flux_web::http::HttpRequestTool`; this file asserts the *projection*, so what
/// the transport is does not matter here beyond it being one.
fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in".into(),
            input_schema: serde_json::json!({"type": "object"}),
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
/// The pack requires one, and this file asserts the *projection* — so it holds nothing, for the same
/// reason the transport is a stand-in. Whether a value happens to be stored must not decide whether
/// the shipped catalogue installs.
fn credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant id")
}

/// The tenant both ports answer for. One constant, because a pack whose two ports name different
/// tenants is refused at install ([`connector_pack::Error::TenantMismatch`]).
const TENANT: &str = "t-projection";

/// A bound configuration port (C-193).
///
/// Installing needs no *values* — projection reads which variables an operation names, never what
/// they are — so this is empty except for the one connector whose **refusals** are asserted below.
/// `zendesk` is configured there so that a call omitting `ticket_id` still refuses by naming
/// `ticket_id`: without a subdomain the first thing missing is the subdomain, which is a true
/// answer to a different question.
fn configuration() -> Configuration {
    let values = MemoryConfig::new().with_endpoint(TENANT, "zendesk", "subdomain", "acme");
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

/// Every provider the catalogue ships, in its own stable order.
fn every_provider() -> Vec<&'static str> {
    catalog::providers()
        .iter()
        .map(|provider| provider.id)
        .collect()
}

/// **The Acceptance test.** Iterate the real catalogue, project each operation, register the lot,
/// and assert the registry resolves each dotted name.
///
/// This is the whole contract in one place: if `pack` does not install, if a name is projected
/// wrongly, or if two operations collide into one registry entry, this fails and names the
/// operation.
#[test]
fn every_shipped_operation_projects_to_a_registrable_spec() {
    let providers = every_provider();
    let mut registry = ToolRegistry::new();

    connector_pack::pack(&providers, http(), credentials(), configuration())(&mut registry)
        .expect("the shipped catalogue installs into an empty registry");

    let mut projected = 0usize;
    for operation in catalog::operations() {
        let dotted = connector_pack::dotted_name(operation.id)
            .unwrap_or_else(|error| panic!("`{}` has no dotted tool name: {error}", operation.id));

        let tool = registry.get(&dotted).unwrap_or_else(|| {
            panic!(
                "`{}` projected to `{dotted}`, which the registry does not resolve",
                operation.id
            )
        });

        let spec = tool.spec();
        assert_eq!(
            spec.name, dotted,
            "`{}` registered under a name its own spec does not carry",
            operation.id
        );
        // The projected description is the declaration's, which is the catalogue's summary extended
        // with the vendor's error envelope where the connector declares one. Equality would be the
        // wrong assertion: it is the *module's* model-facing text the pack has to agree with, or
        // one operation tells a model two different things depending on how it was installed.
        assert!(
            spec.description.starts_with(operation.description),
            "`{}` describes itself as `{}`, which does not begin with the catalogue's `{}`",
            operation.id,
            spec.description,
            operation.description
        );
        projected += 1;
    }

    assert!(
        projected > 0,
        "an empty catalogue would pass every assertion above"
    );
    assert_eq!(
        registry.names().len(),
        projected,
        "the registry holds a different number of tools than the catalogue offers"
    );
}

/// The seam the reference flow depends on, spelled out against the four operations
/// `examples/zendesk.triage.flux` actually calls. If this drifts, that flow stops resolving.
#[test]
fn the_reference_flow_resolves_every_operation_it_calls() {
    let mut registry = ToolRegistry::new();
    connector_pack::pack(&["zendesk"], http(), credentials(), configuration())(&mut registry)
        .expect("zendesk installs");

    for name in [
        "zendesk.test",
        "zendesk.ticket.show",
        "zendesk.ticket.search",
        "zendesk.ticket.comment.list",
    ] {
        assert!(
            registry.get(name).is_some(),
            "the reference flow calls `{name}`, which the pack does not register"
        );
    }
}

/// Acceptance: the projection carries the catalogue's own safety metadata, and does not invent a
/// field the catalogue entry has no value for.
#[test]
fn the_spec_carries_the_catalogue_entry_and_invents_nothing() {
    let mut registry = ToolRegistry::new();
    connector_pack::pack(&["zendesk"], http(), credentials(), configuration())(&mut registry)
        .expect("zendesk installs");

    let operation = catalog::operation(OperationKey::id("zendesk-ticket-comment-add"))
        .expect("the shipped catalogue carries zendesk-ticket-comment-add");
    let spec = registry
        .get("zendesk.ticket.comment.add")
        .expect("the operation registers under its dotted name")
        .spec();

    assert!(spec.description.starts_with(operation.description));
    assert_eq!(spec.risk, flux_spec::Risk::Medium);
    assert_eq!(spec.idempotency, flux_spec::Idempotency::Conditional);
    assert_eq!(spec.effects, vec![flux_spec::Effect::Network]);

    // The declared parameters, as a real JSON Schema — not an empty object standing in for one.
    let properties = spec.input_schema["properties"]
        .as_object()
        .expect("the input schema declares an object's properties");
    assert_eq!(
        properties["ticket_id"],
        serde_json::json!({"type": "number"})
    );
    assert_eq!(properties["body"], serde_json::json!({"type": "string"}));
    assert_eq!(properties["public"], serde_json::json!({"type": "boolean"}));

    // Fields the catalogue entry has no value for are absent rather than fabricated. Every
    // generated op returns `Any`, and no operation declares a tool group.
    assert_eq!(spec.output_schema, None);
    assert_eq!(spec.group, None);
}

/// Acceptance: a collision surfaces flux's own duplicate diagnostic rather than panicking, and the
/// per-provider source label names the contributor.
#[test]
fn a_collision_surfaces_fluxs_duplicate_diagnostic_rather_than_panicking() {
    let mut registry = ToolRegistry::new();
    connector_pack::pack(&["zendesk"], http(), credentials(), configuration())(&mut registry)
        .expect("the first install succeeds");

    // The same provider again: every one of its operations collides with itself.
    let error =
        connector_pack::pack(&["zendesk"], http(), credentials(), configuration())(&mut registry)
            .expect_err("installing the same provider twice must be refused, not silently merged");
    let rendered = error.to_string();

    assert!(
        rendered.contains("zendesk"),
        "the diagnostic must name the colliding operation's provider: {rendered}"
    );

    // `try_register_all_from` is atomic, so the registry is exactly what the first install left.
    assert_eq!(
        registry.names().len(),
        catalog::operations_of(ProviderKey::id("zendesk")).len(),
        "a refused install must leave nothing of itself behind"
    );
}

/// A provider the catalogue does not carry is an error, not an empty install. Registering nothing
/// and reporting success is how a host ends up with a client that silently cannot call anything.
#[test]
fn an_unknown_provider_is_refused_rather_than_installed_as_nothing() {
    let mut registry = ToolRegistry::new();
    // `salesforce` used to be the sentinel and stopped being unknown when C-163 shipped it.
    const NO_SUCH_PROVIDER: &str = "no-such-vendor";

    let error = connector_pack::pack(&[NO_SUCH_PROVIDER], http(), credentials(), configuration())(
        &mut registry,
    )
    .expect_err("an unknown provider must be refused");

    assert!(error.to_string().contains(NO_SUCH_PROVIDER), "{error}");
    assert!(registry.names().is_empty());
}

/// `execute` builds a request and delegates it (C-115), so a call it cannot build must be an error
/// naming the operation and the parameter — never a panic, which in a host process is strictly
/// worse than a failed tool call, and never a partly-assembled request, which the vendor answers.
///
/// Constructing a real [`flux_runtime::ToolContext`] needs a `flux_system::System` over a real
/// workspace root, which is a filesystem-and-environment dependency out of all proportion to what
/// is being asserted — and `execute`'s whole body is `build_request` plus a delegation. So the
/// refusal is asserted where it is decided. `tests/request.rs` covers what a *successful* build
/// produces.
#[test]
fn a_call_that_cannot_be_built_is_refused_by_name_rather_than_panicking() {
    let entry = catalog::operation(OperationKey::id("zendesk-ticket-show"))
        .expect("the shipped catalogue carries zendesk-ticket-show");
    let operation =
        connector_pack::Operation::project(entry, http(), credentials(), configuration())
            .expect("the entry projects");

    let error = operation
        .build_request(&serde_json::json!({}))
        .expect_err("a call with no `ticket_id` is not a request")
        .to_string();

    assert!(error.contains("zendesk-ticket-show"), "{error}");
    assert!(error.contains("ticket_id"), "{error}");
}
