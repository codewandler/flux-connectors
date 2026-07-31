//! Algolia (C-164) is the epic's probe for a value that has to reach **two positions on the same
//! request at once**: `X-Algolia-Application-Id` is a mandatory header on every Algolia REST call,
//! and the *same* application id also forms the request's hostname (`{app_id}-dsn.algolia.net`).
//!
//! C-164 found all three available routes blocked and shipped no `providers/algolia.toml`. **C-187
//! removed one of the three**, and this file is the record of exactly which:
//!
//! 1. **`ConfigField::binds` reaches a header now, without routing through `[[auth]]`.**
//!    `Binding::Request { position: Position::Header, .. }` is a non-secret, connection-level
//!    destination — `header.X-Algolia-Application-Id` parses, its `is_secret()` is `false`, and the
//!    emitted module carries the value as a substitutable placeholder in its header record. That was
//!    C-164's first finding and it no longer holds.
//! 2. **The credential route still forces a lie, and is still the wrong one.** An `[[auth]]`-declared
//!    credential makes `secret = true` unconditional for any `[[config]]` field binding it
//!    (`Binding::is_secret`), and an application id Algolia publishes as safe to embed client-side is
//!    not a secret. Unchanged, and now avoidable rather than merely refused.
//! 3. **One declared value still cannot occupy two positions.** `binds` names exactly *one*
//!    destination, and a header pin's placeholder is its header name while an endpoint binding's is
//!    its template variable — so the hostname and the header are two fields with two placeholders and
//!    two host-side slots. An operator would type the application id twice, with nothing keeping the
//!    two in step, which is precisely the outcome C-164 weighed against a refusal.
//!
//! So the probe's answer is now *narrower* rather than overturned, and this file pins the new
//! boundary with the loader and the emitter rather than leaving it as prose.
//!
//! # C-164 made the call: still no `providers/algolia.toml`, and the space is now closed
//!
//! Finding 3 is not an ergonomic wart to be waived — it is a rule, and the three ways to make the
//! two positions share one value are each measured here rather than argued:
//!
//! | shape | outcome |
//! |---|---|
//! | two fields, different names (`endpoint.app_id` + `header.X-Algolia-Application-Id`) | **loads** — and is the problem: two host-side slots, one answer, nothing keeping them in step |
//! | two fields, one name (both spelled `X-Algolia-Application-Id`) | **refused** — `validate_pin`'s C-197 shared-slot pass |
//! | one field, header pin alone, hostname resolving from it | **refused** — only `Binding::Endpoint` binds a `base_url` variable |
//!
//! The middle row is the sharp one. The declaration that would give Algolia what it needs — one
//! collected value substituted into both positions — is refused *by name*, and the rule refusing it
//! is right: two **fields** keyed to one slot means one field's answer is silently discarded. The
//! rule and the vendor want opposite things, and this model has no way to write the one question
//! that would satisfy both. Algolia is the only one of C-187's three motivating vendors whose scope
//! sits in *two* positions — Cloudflare's `zone_id` and Vercel's `teamId` each sit in one, which is
//! why they ship and this does not.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::config::parse_binding;
use connector_spec::{provider, Binding, Level, Position};

/// A minimal, otherwise-valid provider fixture. Only the pieces under test vary — the rest is held
/// constant so a failure is about the binding, and nothing else.
///
/// `{app_id}` in `base_url` mirrors Algolia's real hostname shape
/// (`https://{app_id}-dsn.algolia.net`), and the API key is declared exactly the way it would ship:
/// a real secret, `Header` scheme, gated as `secret = true`. Only the *application id*'s config
/// block varies across cases below.
fn fixture(application_id_auth: &str, application_id_config: &str) -> String {
    fixture_hosted_on(
        "https://{app_id}-dsn.algolia.net",
        application_id_auth,
        application_id_config,
    )
}

/// [`fixture`] with the hostname template varied too, for the cases that turn on *which* name the
/// `base_url` placeholder and the header pin are spelled with.
fn fixture_hosted_on(
    base_url: &str,
    application_id_auth: &str,
    application_id_config: &str,
) -> String {
    format!(
        r#"
id = "algolia"
vendor = "Algolia"
base_url = "{base_url}"

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

/// **A non-secret, operator-supplied value reaches a request header** (C-187) — C-164's first
/// finding, overturned.
///
/// `header.<name>` parses to `Binding::Request { position: Header, .. }`: connection level, derived
/// rather than authored, and **not secret**, so the `secret`/`binds` agreement that makes the
/// credential path trustworthy is untouched. That is the shape C-164's own note asked for — "a
/// binding that reaches a header *without* routing through `[[auth]]`" — rather than the weakening
/// it ruled out.
#[test]
fn a_config_field_reaches_a_header_without_routing_through_auth() {
    assert_eq!(
        parse_binding("header.X-Algolia-Application-Id"),
        Ok(Binding::Request {
            position: Position::Header,
            name: "X-Algolia-Application-Id"
        })
    );
    assert_eq!(
        parse_binding("header.X-Algolia-Application-Id").map(Binding::level),
        Ok(Level::Connection),
        "an application id is one per tenant, not one per vendor"
    );
    assert_eq!(
        parse_binding("header.X-Algolia-Application-Id").map(Binding::is_secret),
        Ok(false),
        "the whole point: a public identifier reaches a header without being declared a credential"
    );

    // The set is still closed. A destination nobody gave a spelling is still a load error, not a
    // key the loader accepts and ignores.
    let error = parse_binding("cookie.session").expect_err("no such destination exists");
    for known in [
        "endpoint.<variable>",
        "path.<variable>",
        "query.<name>",
        "header.<name>",
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

/// The header pin reaches the wire, not just the declaration: the emitted module binds it as a
/// literal carrying its placeholder and puts that symbol in the request's header record, and it
/// never appears in the operation's parameter list.
#[test]
fn a_pinned_header_reaches_the_emitted_request_and_not_the_signature() {
    let config = r#"
[[config]]
name = "app_id"
label = "Algolia application id"
help = "For the probe fixture only"
binds = "endpoint.app_id"

[[config]]
name = "application_id_header"
label = "Algolia application id (header)"
help = "For the probe fixture only"
binds = "header.X-Algolia-Application-Id"
"#;
    let loaded = provider::load("providers/algolia.toml", &fixture("", config))
        .expect("a non-secret header pin is a legal declaration");
    let operation = &loaded.connector.operations[0];
    let flux = emit_operation(&loaded.connector, operation).expect("it must emit");

    assert!(
        flux.contains(r#"X_Algolia_Application_Id = "{X-Algolia-Application-Id}""#),
        "the pin must be bound as a literal carrying its placeholder:\n{flux}"
    );
    assert!(
        flux.contains(r#""X-Algolia-Application-Id": X_Algolia_Application_Id"#),
        "the pinned symbol must reach the request's header record:\n{flux}"
    );
    assert!(
        flux.starts_with("op algolia-index-list -> Any"),
        "a pinned header is never a caller argument:\n{flux}"
    );
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

/// **One declared value still cannot occupy two positions** — C-164's third finding, unchanged.
///
/// `binds` names exactly one destination, and the two that Algolia needs carry *different*
/// placeholders: an endpoint binding's is its `base_url` template variable (`app_id`), a header
/// pin's is the header name itself (`X-Algolia-Application-Id`). A host keys a configuration value
/// by `(tenant, provider, service, kind, name)`, so those are two slots — two questions in the
/// connect form, both answered with the same string, with nothing enforcing that they agree.
///
/// That is not a defect in the pin; it is the boundary of what C-187 set out to move. It is asserted
/// here so that shipping this connector remains a deliberate decision rather than an oversight.
#[test]
fn the_hostname_and_the_header_are_still_two_declared_fields_with_two_slots() {
    let config = r#"
[[config]]
name = "app_id"
label = "Algolia application id"
help = "For the probe fixture only"
binds = "endpoint.app_id"

[[config]]
name = "application_id_header"
label = "Algolia application id (header)"
help = "For the probe fixture only"
binds = "header.X-Algolia-Application-Id"
"#;
    let loaded = provider::load("providers/algolia.toml", &fixture("", config))
        .expect("both declarations are legal — that is the problem");

    let bindings: Vec<Option<Binding<'_>>> = loaded
        .connector
        .config
        .iter()
        .filter(|field| field.name != "api_key")
        .map(|field| field.binding())
        .collect();
    assert_eq!(
        bindings,
        [
            Some(Binding::Endpoint { variable: "app_id" }),
            Some(Binding::Request {
                position: Position::Header,
                name: "X-Algolia-Application-Id"
            }),
        ],
        "two fields, two binding targets — an operator types the application id twice"
    );

    // And the loader is right not to refuse them: they resolve two *different* placeholders, so
    // they are not the C-197 one-slot collapse the pin rules do refuse. They are simply two
    // questions with one answer, which no declaration here can express as one.
    assert_ne!(
        loaded.connector.config[1].binds,
        loaded.connector.config[2].binds
    );
}

/// **Spelling both destinations with one name — the declaration that would make them share — is
/// refused, by name, at the loader.**
///
/// This is the "why not just…" the previous two findings leave open. If the `base_url` placeholder
/// and the header are given the *same* spelling, the two fields resolve one `{placeholder}` and a
/// host would substitute one collected value into both positions, which is exactly the behaviour
/// Algolia needs. The loader refuses it: `validate_pin`'s shared-slot pass
/// (`crates/connector-spec/src/provider.rs:795-820`) compares a pin's name against every other
/// field's endpoint variable and request name, and rejects a match as the C-197 one-slot collapse.
///
/// The refusal is right on its own terms — two *fields* keyed to one slot means one field's answer
/// is silently discarded — and it is precisely what makes Algolia unshippable. The rule and the
/// vendor want opposite things: the rule says two questions sharing an answer must be one question,
/// and this model has no way to *write* that one question.
#[test]
fn one_name_for_both_destinations_is_refused_as_a_shared_slot() {
    let config = r#"
[[config]]
name = "app_id"
label = "Algolia application id"
help = "For the probe fixture only"
binds = "endpoint.X-Algolia-Application-Id"

[[config]]
name = "application_id_header"
label = "Algolia application id (header)"
help = "For the probe fixture only"
binds = "header.X-Algolia-Application-Id"
"#;
    let source = fixture_hosted_on(
        "https://{X-Algolia-Application-Id}-dsn.algolia.net",
        "",
        config,
    );
    let error = provider::load("providers/algolia.toml", &source)
        .expect_err("two fields resolving one placeholder must be refused");
    let message = error.to_string();
    assert!(
        message.contains("app_id")
            && message.contains("application_id_header")
            && message.contains("one slot"),
        "expected the C-197 shared-slot refusal naming both fields, got: {message}"
    );
}

/// **A header pin does not satisfy a `base_url` template variable**, so the header alone cannot
/// stand in for the hostname either.
///
/// The remaining escape would be to declare the application id *once*, as a header pin, and let the
/// hostname template resolve from it. It does not:
/// `validate_every_template_variable_is_asked_for` (`provider.rs:831-855`) matches only
/// `Binding::Endpoint`, so a `{placeholder}` in a `base_url` is bound by an endpoint binding or by
/// nothing. With the header pin as the sole declaration the connector has no valid destination URL,
/// and the loader says so rather than emitting one.
#[test]
fn a_header_pin_does_not_bind_the_hostname_template() {
    let config = r#"
[[config]]
name = "application_id"
label = "Algolia application id"
help = "For the probe fixture only"
binds = "header.X-Algolia-Application-Id"
"#;
    let source = fixture_hosted_on(
        "https://{X-Algolia-Application-Id}-dsn.algolia.net",
        "",
        config,
    );
    let error = provider::load("providers/algolia.toml", &source)
        .expect_err("a header pin must not count as binding the hostname");
    let message = error.to_string();
    assert!(
        message.contains("X-Algolia-Application-Id") && message.contains("endpoint."),
        "expected the unbound-template refusal pointing at `endpoint.`, got: {message}"
    );
}

/// **The recorded outcome: no dishonest connector was shipped for this probe.**
///
/// `providers/algolia.toml` still does not exist, and C-187 narrowed rather than removed the reason.
/// Mislabelling a public identifier as a secret is no longer necessary — a header pin is non-secret
/// by construction — but asking an operator for the application id **twice**, with no guard against
/// the two answers disagreeing, still is.
///
/// C-164 has now made that call with the space closed rather than surveyed: the unifying declaration
/// is refused (`one_name_for_both_destinations_is_refused_as_a_shared_slot`) and the header pin does
/// not reach the hostname (`a_header_pin_does_not_bind_the_hostname_template`). The remaining shape
/// would ship a second `[[config]]` field for which no honest `label`/`help` can be written — its
/// only truthful help text is "type the same value again" — against a configuration contract that
/// requires a connector to ask for "everything it needs and nothing it cannot use". See the module
/// documentation and the story's `## Progress`.
#[test]
fn no_provider_toml_was_shipped_for_this_probe() {
    let path = providers_dir().join("algolia.toml");
    assert!(
        !path.exists(),
        "providers/algolia.toml exists, but one declared value still cannot reach both the \
         hostname and the header — if this now exists, that finding has been overturned and this \
         test (and the story's `## Progress`) must be updated to say how"
    );
}
