//! **An operator-pinned configuration value reaches the request, and never the signature** — C-187.
//!
//! `ConfigField::binds` used to reach a `{placeholder}` in a service's `base_url` and nothing else,
//! so a tenant scope living anywhere further down the request had to ship as a per-call argument.
//! Two connectors measured that in one wave — Cloudflare's `zone_id` (a path segment) and Vercel's
//! `teamId` (a query parameter) — and a third, Algolia, could not ship at all because its
//! non-secret application id had to reach a *header*.
//!
//! The tests here assert on the **emitted request**, not on whether the provider loads, for the same
//! reason `constant_headers.rs` does: a connector whose pin was silently dropped compiles, formats,
//! round-trips, ships, and then addresses a tenant nobody chose — which the vendor answers `200` to.
//!
//! Fixtures are vendor-shaped rather than a shipped vendor: a mechanism story must not ship a
//! provider to prove itself.

use connector_flux::{emit_operation, Error};
use connector_spec::provider::load;
use connector_spec::{Connector, JsonSchema, Operation, Param};

/// A connector whose operations are scoped under one `{tenant_id}` path segment, with one operation
/// deliberately unscoped — the shape that makes "the call that discovers the value" expressible.
const PINNED_PATH: &str = r#"
id = "vendor"
vendor = "Vendor"
base_url = "https://api.vendor.example"
description = "A vendor whose tenancy is a path segment"

[[operations]]
id = "vendor-tenant-list"
method = "GET"
path = "/v1/tenants"
description = "List the tenants this credential can see."
risk = "low"
idempotency = "idempotent"

[[operations]]
id = "vendor-widget-list"
method = "GET"
path = "/v1/tenants/{tenant_id}/widgets"
description = "List the widgets of the pinned tenant."
risk = "low"
idempotency = "idempotent"

[[config]]
name = "tenant_id"
label = "Tenant"
help = "The tenant this connection manages"
example = "t_12345"
binds = "path.tenant_id"
"#;

/// The Vercel shape: a pinned query parameter, alongside one required and one optional argument, so
/// the `?`/`&` separators are under test too.
const PINNED_QUERY: &str = r#"
id = "vendor"
vendor = "Vendor"
base_url = "https://api.vendor.example"
description = "A vendor whose tenancy is a query parameter"

[[operations]]
id = "vendor-widget-list"
method = "GET"
path = "/v1/widgets"
description = "List widgets."
risk = "low"
idempotency = "idempotent"

[[operations.params.query]]
name = "kind"
description = "Which kind of widget."
required = true
schema = { type = "string" }

[[operations.params.query]]
name = "cursor"
description = "Page cursor."
required = false
schema = { type = "string" }

[[config]]
name = "account_id"
label = "Account"
help = "The account every call acts on behalf of"
example = "acct_12345"
binds = "query.accountId"
"#;

fn connector(source: &str) -> Connector {
    load("providers/fixture.toml", source)
        .unwrap_or_else(|error| panic!("the fixture must load:\n{error}"))
        .connector
}

fn op<'a>(connector: &'a Connector, id: &str) -> &'a Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("no operation named {id:?}"))
}

fn emit(connector: &Connector, id: &str) -> String {
    emit_operation(connector, op(connector, id))
        .unwrap_or_else(|error| panic!("`{id}` must emit: {error}"))
}

/// **The path case.** The pin is bound to a string literal spelling its own placeholder — the same
/// shape a templated `base_url` already has, and the same shape a host substitutes into — and the
/// operation declares no parameter for it.
#[test]
fn a_pinned_path_segment_is_a_literal_placeholder_and_not_an_argument() {
    let connector = connector(PINNED_PATH);
    let flux = emit(&connector, "vendor-widget-list");

    assert!(
        flux.starts_with("op vendor-widget-list -> Any"),
        "a pinned segment must not reach the declared parameter list:\n{flux}"
    );
    assert!(
        flux.contains(r#"tenant_id = "{tenant_id}""#),
        "the pin must be a literal carrying its placeholder, which is what a host substitutes \
         into:\n{flux}"
    );
    assert!(
        flux.contains(r#"url = fmt("{base}/v1/tenants/{tenant_id}/widgets")"#),
        "the URL must interpolate the pinned symbol:\n{flux}"
    );
}

/// A path pin applies only where the placeholder is. The operation that *discovers* the value must
/// stay callable, and it must not acquire a binding it has nowhere to put.
#[test]
fn an_operation_whose_path_carries_no_placeholder_is_untouched() {
    let connector = connector(PINNED_PATH);
    let flux = emit(&connector, "vendor-tenant-list");

    assert!(
        !flux.contains("tenant_id"),
        "an unscoped operation must not bind a pin it cannot use:\n{flux}"
    );
    assert!(flux.contains(r#"url = fmt("{base}/v1/tenants")"#), "{flux}");
}

/// **The query case**, including the separators. A pinned parameter is unconditional — it needs no
/// `when` guard, because a pinned value has no "not supplied" state — and it lands after the
/// caller's required arguments and before the guarded optional ones.
#[test]
fn a_pinned_query_parameter_is_unconditional_and_keeps_the_separators_right() {
    let connector = connector(PINNED_QUERY);
    let flux = emit(&connector, "vendor-widget-list");

    assert!(
        flux.starts_with("op vendor-widget-list(kind: String, cursor: String) -> Any"),
        "only the caller's own arguments are declared:\n{flux}"
    );
    assert!(
        flux.contains(r#"accountId = "{accountId}""#),
        "the pin must be a literal carrying its placeholder:\n{flux}"
    );
    assert!(
        flux.contains(r#"url = fmt("{base}/v1/widgets?kind={kind}&accountId={accountId}")"#),
        "the required argument opens the query string and the pin follows it:\n{flux}"
    );
    assert!(
        flux.contains(r#"sep = "&""#),
        "the query string is already open, so the optional filter hands on `&`:\n{flux}"
    );
    assert!(
        !flux.contains("when accountId"),
        "a pinned value is always sent — a guard would describe a state that cannot occur:\n{flux}"
    );
}

/// A pin with nothing before it opens the query string itself, so the `?` is not lost when an
/// operation declares no required argument of its own.
#[test]
fn a_pin_with_no_required_argument_before_it_opens_the_query_string() {
    let source = PINNED_QUERY.replace(
        r#"
[[operations.params.query]]
name = "kind"
description = "Which kind of widget."
required = true
schema = { type = "string" }
"#,
        "",
    );
    let connector = connector(&source);
    let flux = emit(&connector, "vendor-widget-list");

    assert!(
        flux.contains(r#"url = fmt("{base}/v1/widgets?accountId={accountId}")"#),
        "{flux}"
    );
    assert!(
        flux.contains(r#"sep = "&""#),
        "the pin opened the query string, so the optional filter must hand on `&`:\n{flux}"
    );
}

/// **The refusal that keeps a pin from being advisory**, reached the only way it can be: the loader
/// refuses this shape in a provider file, so the IR is built and then perturbed — exactly as
/// `cloudflare_connector.rs` does for the `POST`/idempotency rule.
///
/// The emitter is an independent gate over an IR that another front-end could produce, and this is
/// the one shape where honouring either side is worse than refusing: honour the parameter and the
/// operator's pin is decoration, honour the pin and a caller's value vanishes without a word.
#[test]
fn a_slot_claimed_by_both_a_pin_and_a_parameter_is_refused_at_emission() {
    let mut connector = connector(PINNED_PATH);
    let schema: JsonSchema = serde_json::json!({ "type": "string" });
    let operation = connector
        .operations
        .iter_mut()
        .find(|operation| operation.id == "vendor-widget-list")
        .expect("declared");
    operation.params.path.push(Param {
        name: "tenant_id".to_string(),
        description: "The tenant, as a caller argument.".to_string(),
        required: true,
        schema,
        ..Param::default()
    });

    let error = emit_operation(&connector, op(&connector, "vendor-widget-list"))
        .expect_err("a pinned value that is also a parameter must be refused");
    assert!(
        matches!(&error, Error::PinnedValueConflict { position, name, .. }
            if *position == "path" && name == "tenant_id"),
        "expected the pin/parameter conflict, got: {error}"
    );
    assert!(
        error.to_string().contains("declare it on one side only"),
        "the refusal must name the fix: {error}"
    );
}

/// Every emitted module still parses and is a fixed point of flux's own formatter — the standing
/// per-provider gate, applied to the shape this story adds.
#[test]
fn every_pinned_module_parses_and_is_canonically_formatted() {
    for source in [PINNED_PATH, PINNED_QUERY] {
        let connector = connector(source);
        for operation in &connector.operations {
            let flux = emit(&connector, &operation.id);
            let parsed = flux_lang::parser::parse_cst(&flux);
            assert!(
                parsed.errors.is_empty(),
                "`{}` emits Flux that does not parse: {:?}\n{flux}",
                operation.id,
                parsed.errors
            );
            assert_eq!(
                flux_lang::format_cst::format_module(&parsed).as_deref(),
                Some(flux.as_str()),
                "the flux formatter would rewrite `{}`",
                operation.id
            );
        }
    }
}
