//! **C-214: a configuration value is checked where it is *substituted*, not only where it is
//! declared.**
//!
//! `connector-spec` has had the predicate since C-187 — `Position::validate_value` — and it has two
//! call sites, both in the loader, both running against a `ConfigField::example` or a parameter
//! *name*. Nothing ran against the value a tenant actually supplies, because this repository never
//! sees one: it arrives through [`ConfigStore`], at the host, at request time. So the guard existed
//! and did not guard.
//!
//! The severe half is the **host** position, and it predates the pin surface C-187 added. A path or
//! query value cannot move the origin — substitution lands after the authority is fixed in the
//! `base` literal — but nine shipped connectors template the *host* itself, and there the `@` in
//! `acme.zendesk.com@evil.example` turns everything before it into userinfo:
//!
//! ```text
//! https://acme.zendesk.com@evil.example.zendesk.com/api/v2/tickets/1.json
//!         └──────── userinfo ─────┘└─── the host that is actually resolved ───┘
//! ```
//!
//! The value is operator-supplied rather than attacker-supplied, so this is a paste-the-wrong-thing
//! hazard rather than a classic injection. What makes it worth a refusal anyway is where the wrong
//! thing goes: a host the operator never named, carrying that operator's own token, through an
//! egress gate that was shown a subject ending in `zendesk.com`.

use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{
    Configuration, Credentials, Egress, MemoryConfig, MemoryStore, Operation, Request,
};
use serde_json::json;

/// The tenant both ports answer for.
const TENANT: &str = "t-c214";

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

fn credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant id")
}

/// A configuration port holding exactly the `(provider, service, variable, value)` rows given.
fn configured(rows: &[(&str, &str, &str, &str)]) -> Configuration {
    let mut values = MemoryConfig::new();
    for (provider, service, variable, value) in rows {
        values = values.with_endpoint(TENANT, provider, service, variable, value);
    }
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

fn projected(id: &str, configuration: Configuration) -> Operation {
    let entry = catalog::operation(OperationKey::id(id)).expect("the shipped catalogue carries it");
    Operation::project(entry, http(), credentials(), configuration)
        .unwrap_or_else(|error| panic!("`{id}`: {error}"))
}

/// The host a URL actually resolves to: the authority, minus any userinfo, minus any port.
///
/// Hand-rolled rather than taken from a URL crate, because the point of the assertion is to read
/// the URL the way a transport does and this crate deliberately links none.
fn resolved_host(url: &str) -> &str {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let authority = after_scheme
        .find(['/', '?', '#'])
        .map_or(after_scheme, |end| &after_scheme[..end]);
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    host.rsplit_once(':')
        .filter(|(_, port)| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
        .map_or(host, |(host, _)| host)
}

/// The request an operation builds, or the refusal it produced — reported as the URL it would have
/// sent and the host that URL actually resolves to, which is the fact the whole story is about.
fn built(operation: &Operation, params: serde_json::Value) -> Result<Request, String> {
    operation
        .build_request(&params)
        .map_err(|error| error.to_string())
}

/// **The failing-first test.** A `subdomain` of `acme.zendesk.com@evil.example` must not produce a
/// request whose authority is `evil.example.zendesk.com`.
///
/// Before this story `zendesk-ticket-show` built that URL and returned it happily, so the assertion
/// is written as "the request was refused, and the refusal says which field and which operation" —
/// the two facts an operator needs in order to go and fix the value they pasted.
#[test]
fn a_host_value_cannot_move_the_origin() {
    let operation = projected(
        "zendesk-ticket-show",
        configured(&[(
            "zendesk",
            "default",
            "subdomain",
            "acme.zendesk.com@evil.example",
        )]),
    );

    let refusal = match built(&operation, json!({"ticket_id": 1})) {
        Ok(request) => panic!(
            "the request was built and it resolves to `{}`, not to a zendesk subdomain: {}",
            resolved_host(&request.url),
            request.url
        ),
        Err(refusal) => refusal,
    };

    assert!(
        refusal.contains("zendesk-ticket-show"),
        "the refusal does not name the operation: {refusal}"
    );
    assert!(
        refusal.contains("subdomain"),
        "the refusal does not name the field: {refusal}"
    );
}

/// A value that is a subdomain and nothing else still works, which is the half that makes the guard
/// a guard rather than a ban. Nine connectors template their host; all of them must keep resolving.
#[test]
fn an_ordinary_subdomain_still_resolves() {
    let operation = projected(
        "zendesk-ticket-show",
        configured(&[("zendesk", "default", "subdomain", "acme")]),
    );

    let request = built(&operation, json!({"ticket_id": 1})).expect("`acme` is a subdomain");
    assert_eq!(resolved_host(&request.url), "acme.zendesk.com");
    assert_eq!(
        request.url,
        "https://acme.zendesk.com/api/v2/tickets/1.json"
    );
}
