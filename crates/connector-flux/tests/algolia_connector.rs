//! Algolia (C-164) is the epic's probe for a value that has to reach **two positions on the same
//! request at once**: `X-Algolia-Application-Id` is a mandatory header on every Algolia REST call,
//! and the *same* application id also forms the request's hostname
//! (`{app_id}-dsn.algolia.net`). Two configured hosts had already shipped by the time this story
//! ran (Salesforce's `{instance}`, C-163) and two credentials on one request had already shipped
//! (Datadog, C-160) — this connector needed both of those *and* one more fact neither needed: the
//! same operator-supplied value in two places.
//!
//! This is not a per-provider contract test in the shape every sibling in this directory is: there
//! is no `providers/algolia.toml` to load, because **no declared value can reach both positions
//! honestly**. That is the answer this probe was chosen to produce — see the story's `## Progress`
//! for the full account — and this file pins the finding down with the loader itself rather than
//! leaving it as prose:
//!
//! 1. **`ConfigField::binds` parses to exactly one of five destinations, and none of them is a
//!    request header.** `Binding` (`crates/connector-spec/src/config.rs:178-202`) is `Endpoint`,
//!    `Credential`, `Username`, `OAuthClientId` or `OAuthClientSecret`; `parse_binding`
//!    (`config.rs:239-267`) accepts only `endpoint.`, `credential.`, `username.`,
//!    `oauth.client_id`, `oauth.client_secret` and refuses everything else, including a
//!    `header.`-shaped destination that does not exist.
//! 2. **The one route that *can* reach a header — an `[[auth]]`-declared credential — forces
//!    `secret = true` on whatever `[[config]]` field binds it**, unconditionally
//!    (`crates/connector-spec/src/provider.rs:609-629`, `Binding::is_secret`,
//!    `config.rs:223-231`). The application id is not a secret — Algolia's own docs publish it
//!    alongside a search-only key as safe to embed in client-side code — so declaring it this way
//!    would be the exact "a field claiming otherwise" case `config.rs`'s own module docs warn
//!    against, not a workaround.
//! 3. **A caller-supplied header parameter (`ParamSet::header`, `crates/connector-spec/src/ir.rs:259-266`)
//!    is a *per-call* argument a model fills in on every invocation** — it has no connection to
//!    `[[config]]` at all, so pinning the application id there does not pin it anywhere; it only
//!    gives the operator a second place to type the same value, with nothing to keep the two in
//!    step.
//!
//! So the two positions genuinely cannot share one declared value today: the endpoint binding
//! reaches the hostname and nothing else, and every route that reaches a header either does not
//! exist (a non-secret header binding) or requires mislabelling a public identifier as a secret.
//! Filed as a finding for
//! [C-187](../../../docs/stories/C-187-config-cannot-pin-a-request-component.md), which already
//! tracks the config surface's reach into a path segment and a query parameter — this is the same
//! gap, met at a header instead.

use std::path::{Path, PathBuf};

use connector_spec::config::parse_binding;
use connector_spec::{provider, Binding};

/// A minimal, otherwise-valid provider fixture. Only the pieces under test vary — the rest is held
/// constant so a failure is about the binding, and nothing else.
///
/// `{app_id}` in `base_url` mirrors Algolia's real hostname shape
/// (`https://{app_id}-dsn.algolia.net`), and the API key is declared exactly the way it would ship:
/// a real secret, `Header` scheme, gated as `secret = true`. Only the *application id*'s config
/// block varies across cases below.
fn fixture(application_id_auth: &str, application_id_config: &str) -> String {
    format!(
        r#"
id = "algolia"
vendor = "Algolia"
base_url = "https://{{app_id}}-dsn.algolia.net"

default_auth = [{{ credentials = ["algolia.api_key"] }}]

[[auth]]
name = "algolia.api_key"
scheme = {{ header = {{ name = "X-Algolia-API-Key" }} }}
env = ["ALGOLIA_API_KEY"]
description = "Algolia Admin/Search API key, for the probe fixture only"

{application_id_auth}

[[operations]]
id = "algolia-index-list"
method = "GET"
path = "/1/indexes"
risk = "low"
idempotency = "idempotent"
description = "List indices, for the probe fixture only"

[[config]]
name = "api_key"
label = "Algolia API key"
help = "For the probe fixture only"
format = "token"
secret = true
binds = "credential.algolia.api_key"

{application_id_config}
"#
    )
}

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// **`Binding` is closed to five destinations, and a request header is not one of them.**
///
/// `crates/connector-spec/src/config.rs:178-202` declares
/// `enum Binding { Endpoint { variable }, Credential { name }, Username { name }, OAuthClientId,
/// OAuthClientSecret }`. `parse_binding` (`:239-267`) accepts only the four string prefixes and two
/// literals that name those five variants and refuses everything else — including a `header.`
/// destination this connector would need, which was never given a spelling to refuse or accept.
#[test]
fn config_binding_has_no_header_destination() {
    let error =
        parse_binding("header.X-Algolia-Application-Id").expect_err("no header binding exists");
    assert!(
        error.contains("is not a binding"),
        "expected the closed-set refusal message, got: {error}"
    );
    for known in [
        "endpoint.<variable>",
        "credential.<name>",
        "username.<name>",
        "oauth.client_id",
        "oauth.client_secret",
    ] {
        assert!(
            error.contains(known),
            "expected the refusal to list every real destination including {known:?}, got: {error}"
        );
    }
}

/// **The one route that reaches a header — a declared `[[auth]]` credential — forces `secret =
/// true`, and the application id is not a secret.**
///
/// Binding the application id to a credential (`Binding::Credential`, whose `is_secret()` is always
/// `true` — `config.rs:223-231`) is the only way this model places a value into an arbitrary
/// request header. The loader enforces the agreement unconditionally
/// (`crates/connector-spec/src/provider.rs:609-629`): a `[[config]]` field binding a credential
/// while declaring `secret = false` is refused, naming exactly this contradiction. Declaring
/// `secret = true` instead would be the dishonest fix the story explicitly rules out — the
/// application id is meant to be readable back, logged, and shown in a UI, none of which a secret
/// field permits.
#[test]
fn routing_the_application_id_through_a_credential_forces_a_false_secret_claim() {
    let auth = r#"
[[auth]]
name = "algolia.application_id"
scheme = { header = { name = "X-Algolia-Application-Id" } }
env = ["ALGOLIA_APPLICATION_ID"]
description = "Algolia application id, for the probe fixture only"
"#;
    let config = r#"
[[config]]
name = "application_id"
label = "Algolia application id"
help = "For the probe fixture only"
secret = false
binds = "credential.algolia.application_id"
"#;
    let source = fixture(auth, config);
    let error = provider::load("providers/algolia.toml", &source)
        .expect_err("binding a credential while declaring secret = false must be refused");
    let message = error.to_string();
    assert!(
        message.contains("application_id") && message.contains("secret = false"),
        "expected the secret/binds agreement error naming the field, got: {message}"
    );
}

/// **The endpoint binding reaches the hostname only — never a header — and a caller-supplied
/// header parameter has no link back to it.**
///
/// Binding the application id to `endpoint.app_id` loads cleanly and correctly resolves the
/// `{app_id}` template variable in `base_url`. But `ParamSet::header`
/// (`crates/connector-spec/src/ir.rs:259-266`) is caller-supplied — a value a *model* fills in on
/// every call, not one a `[[config]]` field can reach — so declaring the same header as an
/// operation parameter does not pin it to the config value at all. It only gives an operator (or a
/// model acting for one) a second, disconnected place to repeat the same string, with nothing
/// enforcing that the two ever agree.
#[test]
fn the_endpoint_binding_reaches_only_the_host_and_a_header_parameter_is_a_separate_per_call_value()
{
    let config = r#"
[[config]]
name = "app_id"
label = "Algolia application id"
help = "For the probe fixture only"
secret = false
binds = "endpoint.app_id"
"#;
    let source = fixture("", config);
    let loaded = provider::load("providers/algolia.toml", &source)
        .expect("binding `endpoint.app_id` to the host template is legal on its own");
    let field = loaded
        .connector
        .config
        .iter()
        .find(|f| f.name == "app_id")
        .expect("the fixture declares it");
    assert_eq!(
        field.binding(),
        Some(Binding::Endpoint { variable: "app_id" })
    );

    // Nothing about this connector's declared operations, or its config, names a route from this
    // binding to a request header. A header parameter reaching `X-Algolia-Application-Id` would
    // have to be declared separately on every operation and filled in by a caller each time —
    // exactly the "ask the operator for the same value twice" outcome the story weighs against a
    // refusal, and the mismatch between the two is a vendor-side 4xx neither half of this pipeline
    // would explain.
    assert!(
        loaded
            .connector
            .operations
            .iter()
            .all(|op| op.params.header.is_empty()),
        "the probe fixture declares no header parameter — the point being that nothing here could \
         connect one to the `app_id` config field even if it did"
    );
}

/// **The recorded outcome: no dishonest connector was shipped for this probe.**
///
/// `providers/algolia.toml` does not exist. See the story's `## Progress` for the full account. If
/// a future story adds one, it must do so only once the config surface can pin a non-secret value
/// into a request header (C-187) — shipping today would mean either asking an operator for the
/// application id twice with no guard against the two disagreeing, or mislabelling a public
/// identifier as a secret.
#[test]
fn no_provider_toml_was_shipped_for_this_probe() {
    let path = providers_dir().join("algolia.toml");
    assert!(
        !path.exists(),
        "providers/algolia.toml exists, but C-164 concluded the application id cannot honestly \
         reach both the hostname and the header from one declared value — if this now exists, the \
         story's refusal has been overturned and this test (and the story's `## Progress`) must be \
         updated to say how"
    );
}
