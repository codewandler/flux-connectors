//! The IR types reject keys they do not know, and refuse to guess a credential's scheme.
//!
//! These tests deliberately use **only** the IR types and `toml`, never the provider loader. That is
//! the point: C-2's review proved the hole is in the types, not in the front-end, and a loader that
//! wrapped permissive types could not close it — a typo'd key would be swallowed by the derived
//! `Deserialize` long before any loader-level check ran. Every assertion here therefore has to hold
//! of `Operation`, `AuthMethod` and `Connector` themselves.
//!
//! # The direction of the failure
//!
//! The hole is not "a typo is ignored". It is "a typo is ignored **towards sending credentials**":
//! a mistyped `authh` on an operation deserializes to `auth: None`, and `None` means *inherit the
//! connector default*, so the operation quietly authenticates with the connector's default
//! credentials instead of the (probably narrower) set the author was trying to name. Failing open on
//! an auth declaration is the worst available default, which is why this is a test and not a lint.

use connector_spec::{AuthMethod, Connector, Operation};

/// An operation whose `auth` key is misspelled. Everything else about it is valid.
const OPERATION_WITH_TYPOED_AUTH: &str = r#"
id = "zendesk.ticket.show"
method = "GET"
path = "/api/v2/tickets/{ticket_id}.json"
risk = "low"
idempotency = "idempotent"
authh = []
"#;

/// A complete operation except for the vendor-state direction C-516 makes mandatory.
const OPERATION_WITHOUT_DIRECTION: &str = r#"
id = "zendesk.ticket.show"
method = "GET"
path = "/api/v2/tickets/{ticket_id}.json"
risk = "low"
idempotency = "idempotent"
"#;

/// A credential whose `env` key is misspelled.
const CREDENTIAL_WITH_TYPOED_ENV: &str = r#"
name = "zendesk.api_token"
scheme = "bearer"
envv = ["ZENDESK_API_TOKEN"]
"#;

/// The headline of C-2's review: a misspelled `auth` must not deserialize to "unset", because unset
/// inherits the connector's default credentials. The failure direction is credential-*sending*.
#[test]
fn a_typoed_operation_auth_key_is_rejected() {
    let parsed = toml::from_str::<Operation>(OPERATION_WITH_TYPOED_AUTH);

    let error = parsed.expect_err(
        "`authh` must be rejected: deserializing it to `auth: None` makes the operation inherit the \
         connector's default credentials, so a one-character typo silently widens what the request \
         sends",
    );
    assert!(
        error.to_string().contains("authh"),
        "the error must name the offending key so the author can find it, got: {error}"
    );
}

/// HTTP method is transport syntax, not proof of whether vendor state changes. Omitting the
/// reviewed direction must therefore fail at the IR boundary before any artifact can be emitted.
#[test]
fn an_operation_must_declare_its_vendor_state_direction() {
    let parsed = toml::from_str::<Operation>(OPERATION_WITHOUT_DIRECTION);

    let error = parsed.expect_err(
        "an operation with no `direction` must be rejected rather than classified from its method",
    );
    assert!(
        error.to_string().contains("direction"),
        "the error must name the missing field, got: {error}"
    );
}

/// The same hole one field down: `envv` deserializes to an empty `env` list, so the credential
/// resolves from nothing and the failure surfaces at request time rather than at build time.
#[test]
fn a_typoed_credential_env_key_is_rejected() {
    let parsed = toml::from_str::<AuthMethod>(CREDENTIAL_WITH_TYPOED_ENV);

    let error = parsed
        .expect_err("`envv` must be rejected rather than yielding a credential with no env keys");
    assert!(
        error.to_string().contains("envv"),
        "the error must name the offending key, got: {error}"
    );
}

/// A credential's scheme decides how the secret reaches the wire. Defaulting it to `bearer` on
/// omission is a safety decision made by silence — the same reasoning `Risk` and `Idempotency` are
/// already documented under — so the field is mandatory on deserialization.
#[test]
fn a_credential_must_declare_its_scheme() {
    let parsed = toml::from_str::<AuthMethod>(
        r#"
name = "zendesk.api_token"
env = ["ZENDESK_API_TOKEN"]
"#,
    );

    let error = parsed
        .expect_err("a credential with no `scheme` must be rejected, not silently made a bearer");
    assert!(
        error.to_string().contains("scheme"),
        "the error must name the missing field, got: {error}"
    );
}

/// Strictness has to reach nested tables too — a typo inside `[operations.quirks]` or
/// `[[operations.params.query]]` is exactly as silent as one at the top level.
#[test]
fn typoed_keys_are_rejected_at_every_nesting_depth() {
    let cases = [
        // Connector level.
        (
            r#"
id = "z"
base_url = "https://z.test"
vendorr = "Zendesk"
"#,
            "vendorr",
        ),
        // Inside a parameter.
        (
            r#"
id = "z"
base_url = "https://z.test"

[[operations]]
id = "z.list"
method = "GET"
direction = "read"
path = "/x"
risk = "low"
idempotency = "idempotent"

[[operations.params.query]]
name = "page"
requiredd = true
schema = { type = "integer" }
"#,
            "requiredd",
        ),
        // Inside quirks.
        (
            r#"
id = "z"
base_url = "https://z.test"

[[operations]]
id = "z.list"
method = "GET"
direction = "read"
path = "/x"
risk = "low"
idempotency = "idempotent"

[operations.quirks.rate_limit]
requests = 100
per_secondss = 60
"#,
            "per_secondss",
        ),
    ];

    for (source, typo) in cases {
        let error = toml::from_str::<Connector>(source)
            .expect_err("a typo'd key at any depth must be rejected, offending key: {typo}");
        assert!(
            error.to_string().contains(typo),
            "the error must name `{typo}`, got: {error}"
        );
    }
}

/// The strictness must not cost the IR its round-trip: a `Connector` re-read from its own encoding
/// still loads. This is the guard against closing the hole by making the types unusable.
#[test]
fn strictness_does_not_break_the_ir_round_trip() {
    let source = r#"
id = "zendesk"
vendor = "Zendesk"
base_url = "https://{tenant}.zendesk.com"

[[auth]]
name = "zendesk.api_token"
scheme = "basic"
env = ["ZENDESK_API_TOKEN"]
user_env = ["ZENDESK_USER"]

[[operations]]
id = "zendesk.ticket.show"
method = "GET"
direction = "read"
path = "/api/v2/tickets/{ticket_id}.json"
risk = "low"
idempotency = "idempotent"
"#;

    let connector: Connector = toml::from_str(source).expect("a valid provider TOML must load");
    let encoded = connector.canonical_json().expect("serialize");
    let decoded: Connector = serde_json::from_str(&encoded).expect("the IR must still round-trip");
    assert_eq!(connector, decoded);
}
