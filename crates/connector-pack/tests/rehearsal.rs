//! **A connector that is not in the index, exercised anyway** (C-233).
//!
//! The gap this closes is structural rather than an oversight. Every other `connector-pack` entry
//! point needs a `&'static catalog::Operation`; `catalog::Operation` is `#[non_exhaustive]`, so no
//! synthetic one can be built outside the `catalog` crate; and the index that carries a real one is
//! a whole-catalogue artifact a story implementor is fenced away from and that does not name a new
//! provider until integration. Measured at this story's base, an attempt to write one is:
//!
//! ```text
//! error[E0639]: cannot create non-exhaustive struct using struct expression
//!   --> crates/connector-pack/tests/rehearsal.rs:32:40
//!    |
//! 32 |       static ENTRY: catalog::Operation = catalog::Operation {
//! ```
//!
//! [`Rehearsal`] takes the operation's **emitted Flux** instead, which a scoped
//! `flux-connectors build --provider <id>` writes to `crates/catalog/ops/<provider>/<id>.flux`. No
//! index, no synthetic catalogue entry, and therefore nothing `#[non_exhaustive]` was protecting
//! that is reopened — see the module documentation on [`Rehearsal`] for the full argument.
//!
//! The whole-catalogue half of the same capability is
//! `request.rs::every_declared_operation_composes_a_request_from_its_declared_configuration`, which
//! runs this over every provider present on disk without anyone writing a test for it.

use std::sync::Arc;

use connector_pack::{Configuration, Error, MemoryConfig, Rehearsal};
use serde_json::json;

/// The tenant every port here answers for.
const TENANT: &str = "t-rehearsal";

/// **C-110's `linear-viewer`, exactly as the emitter wrote it.** The known positive: this shipped,
/// passed a fully green `cargo test --workspace`, and had zero callable operations. The provider
/// file it came from is preserved as a fixture in
/// `crates/connector-flux/tests/linear_connector.rs`.
const LINEAR_VIEWER: &str = r#"op linear-viewer -> Any
  description "Read the user this key belongs to"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """query Viewer {
  viewer {
    id
    name
    displayName
    email
    admin
  }
}
"""
  payload = { query }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
"#;

/// The second of C-110's eight, which also takes a caller parameter — so the refusal is not an
/// artefact of an operation that declares none.
const LINEAR_ISSUE_GET: &str = r#"op linear-issue-get(id: String) -> Any
  description "Read one issue by id"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """query Issue($id: String!) {
  issue(id: $id) {
    id
    identifier
    title
  }
}
"""
  payload = { query, variables: { id } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
"#;

/// A well-shaped connector that is *also* not in the index — the positive control.
///
/// Without it this file would prove only that everything refuses, which a route that refused
/// unconditionally would satisfy just as well. Its shape is the emitter's own: a templated base URL
/// bound as a literal, the path assembled with `fmt`, one `http.request`.
const PROBE_THING_GET: &str = r#"op probe-thing-get(thing_id: Number) -> Any
  description "Show one thing"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.probe.example"
  url = fmt("{base}/api/v1/things/{thing_id}.json")
  response = http.request(method: "GET", url)
  return response
"#;

/// A path pin whose stored value is the Basic credential's non-secret username half (C-475).
const USERNAME_PIN_GET: &str = r#"op twilio-recording-get(Sid: String) -> Any
  description "Fetch one recording"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  AccountSid = "{username.twilio.basic_auth}"
  url = fmt("{base}/Accounts/{AccountSid}/Recordings/{Sid}.json")
  response = http.request(method: "GET", url)
  return response
"#;

fn configuration(rows: &[(&str, &str, &str)]) -> Configuration {
    let mut values = MemoryConfig::new();
    for (provider, service, variable) in rows {
        values = values.with_endpoint(TENANT, provider, service, variable, "acme");
    }
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

/// **The question a provider story could not ask, asked.**
///
/// Both halves of C-110's finding are visible here without a single whole-catalogue artifact being
/// regenerated:
///
/// 1. the operation is refused, rather than composing a request nobody can call;
/// 2. the refusal quotes the literal, so the implementor is looking at their own query document
///    rather than at a variable named after a fragment of it.
#[test]
fn a_connector_that_is_not_in_the_index_can_be_rehearsed_and_refuses() {
    for (operation, flux) in [
        ("linear-viewer", LINEAR_VIEWER),
        ("linear-issue-get", LINEAR_ISSUE_GET),
    ] {
        let error = Rehearsal::of(operation, "linear", "default", flux)
            .err()
            .unwrap_or_else(|| panic!("`{operation}` was rehearsed without refusing"));

        assert!(
            matches!(error, Error::Unbuildable { .. }),
            "`{operation}` refused, but for the wrong reason: {error}"
        );
        let message = error.to_string();
        assert!(
            message.contains("query"),
            "the refusal must quote the document it could not classify: {message}"
        );
    }
}

/// The positive control: the same route, a connector that is genuinely fine, and a real request.
///
/// Its configuration is the one field a provider file for it would declare — `endpoint.subdomain`,
/// keyed under the operation's own service — and nothing else. Compare
/// `every_declared_operation_composes_a_request_from_its_declared_configuration`, which does exactly
/// this over every provider on disk.
#[test]
fn a_connector_that_is_not_in_the_index_composes_a_request_from_its_declared_configuration() {
    let rehearsal = Rehearsal::of("probe-thing-get", "probe", "default", PROBE_THING_GET)
        .expect("a well-shaped operation rehearses");

    assert_eq!(rehearsal.endpoint_variables(), ["subdomain"]);
    assert_eq!(rehearsal.spec().name, "probe.thing.get");

    let request = rehearsal
        .request(
            &configuration(&[("probe", "default", "subdomain")]),
            &json!({"thing_id": 7}),
        )
        .expect("with its declared configuration supplied, the request composes");

    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        "https://acme.probe.example/api/v1/things/7.json"
    );
}

/// **Unconfigured, the same operation refuses by name** — the production shape for a tenant who has
/// not filled the settings form in yet, and the refusal an implementor should see rather than a
/// request to `https://{subdomain}.probe.example`.
#[test]
fn an_unconfigured_rehearsal_refuses_by_name_rather_than_composing_a_placeholder() {
    let rehearsal = Rehearsal::of("probe-thing-get", "probe", "default", PROBE_THING_GET)
        .expect("a well-shaped operation rehearses");

    let error = rehearsal
        .request(&configuration(&[]), &json!({"thing_id": 7}))
        .expect_err("no value was supplied for `subdomain`");

    assert!(
        matches!(&error, Error::MissingConfig { field, .. } if field == "endpoint.subdomain"),
        "{error}"
    );
}

/// A value stored under the wrong **service** is as invisible here as in production (C-197), so a
/// rehearsal cannot pass on a value a host would never have found.
#[test]
fn a_value_stored_under_another_service_does_not_answer() {
    let rehearsal = Rehearsal::of("probe-thing-get", "probe", "management", PROBE_THING_GET)
        .expect("a well-shaped operation rehearses");

    let error = rehearsal
        .request(
            &configuration(&[("probe", "delivery", "subdomain")]),
            &json!({"thing_id": 7}),
        )
        .expect_err("`delivery`'s value must not answer for `management`");

    assert!(matches!(error, Error::MissingConfig { .. }), "{error}");
}

#[test]
fn a_qualified_username_pin_uses_the_basic_username_slot() {
    let rehearsal = Rehearsal::of(
        "twilio-recording-get",
        "twilio",
        "default",
        USERNAME_PIN_GET,
    )
    .expect("a qualified username pin rehearses");
    assert_eq!(
        rehearsal.endpoint_variables(),
        ["username.twilio.basic_auth"]
    );

    let values = MemoryConfig::new().with_username(
        TENANT,
        "twilio",
        "default",
        "twilio.basic_auth",
        "AC00000000000000000000000000000000",
    );
    let configured = Configuration::new(Arc::new(values), TENANT).expect("a valid tenant");
    let request = rehearsal
        .request(
            &configured,
            &json!({"Sid": "RE00000000000000000000000000000000"}),
        )
        .expect("the username-backed path pin composes");
    assert_eq!(
        request.url,
        "https://api.twilio.com/2010-04-01/Accounts/AC00000000000000000000000000000000/Recordings/RE00000000000000000000000000000000.json"
    );

    let missing = rehearsal
        .request(
            &Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant"),
            &json!({"Sid": "RE00000000000000000000000000000000"}),
        )
        .expect_err("the username value is mandatory");
    assert!(
        matches!(&missing, Error::MissingConfig { field, .. } if field == "username.twilio.basic_auth"),
        "{missing}"
    );

    let unsafe_values = MemoryConfig::new().with_username(
        TENANT,
        "twilio",
        "default",
        "twilio.basic_auth",
        "AC00000000000000000000000000000000/Recordings",
    );
    let unsafe_configuration =
        Configuration::new(Arc::new(unsafe_values), TENANT).expect("a valid tenant");
    let unsafe_error = rehearsal
        .request(
            &unsafe_configuration,
            &json!({"Sid": "RE00000000000000000000000000000000"}),
        )
        .expect_err("a username used as a path pin cannot reshape the path");
    assert!(
        matches!(unsafe_error, Error::UnsafeConfig { .. }),
        "{unsafe_error}"
    );
}
