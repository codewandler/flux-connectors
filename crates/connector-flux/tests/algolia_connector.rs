//! Algolia (C-164) is the epic's probe for a value that has to reach **two positions on the same
//! request at once**: `X-Algolia-Application-Id` is a mandatory header on every Algolia REST call,
//! and the *same* application id also forms the request's hostname.
//!
//! The probe's answer was **no**, twice, and both refusals were right. This file is the record of
//! what each one measured and of what removed it:
//!
//! 1. **A `[[config]]` field could not reach a header at all** (C-164, first attempt). The only
//!    route into an arbitrary request header was an `[[auth]]`-declared credential, whose
//!    `Binding::is_secret` is unconditionally `true`, so shipping meant labelling a public
//!    identifier a secret. **C-187 removed it**: `Binding::Request { position: Position::Header }`
//!    is a non-secret, connection-level destination.
//! 2. **One declared value could not reach two positions** (C-164, second attempt). `binds` named
//!    exactly one destination, so the hostname and the header were two fields with two host-side
//!    slots and one answer — an operator typing the application id twice with nothing keeping the
//!    two in step, and a second field whose only truthful `help` was "type the same value again".
//!    Spelling both destinations with *one* name was refused by `validate_pin`'s C-197 shared-slot
//!    pass: **two questions that share an answer are one question**. **C-229 removed it**, by making
//!    the one question writable — `also_binds`.
//!
//! # `providers/algolia.toml` ships, and this file is now about the connector rather than the gap
//!
//! The declaration under test is one field: `binds = "endpoint.app_id"`, `also_binds =
//! ["header.X-Algolia-Application-Id"]`. One `name`, one `label`, one `help`, one row in a connect
//! form, one host-side slot — reaching the authority and the header. `the_application_id_is_one_
//! question_reaching_two_positions` is the acceptance assertion, and
//! `the_two_destinations_carry_one_placeholder_into_the_emitted_module` is what stops a later edit
//! from splitting it back into two slots.
//!
//! **C-164's two boundary measurements are kept alive rather than deleted**, because the rule they
//! pin is what C-229 had to preserve. `one_name_for_both_destinations_is_refused_as_a_shared_slot`
//! asserts that two *fields* under one name are still refused — declaring one question with two
//! destinations is a different statement and must not reopen that door — and
//! `a_header_pin_does_not_bind_the_hostname_template` asserts that a request position still does not
//! satisfy a `base_url` variable, which is why the `endpoint.` destination is `binds` rather than an
//! `also_binds` entry.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::config::parse_binding;
use connector_spec::{provider, Binding, Connector, Level, Pin, Position, Risk};

/// A minimal, otherwise-valid provider fixture. Only the pieces under test vary — the rest is held
/// constant so a failure is about the binding, and nothing else.
///
/// `{app_id}` in `base_url` mirrors Algolia's real hostname shape, and the API key is declared
/// exactly the way it ships: a real secret, `Header` scheme, gated as `secret = true`. Only the
/// *application id*'s config block varies across the cases below.
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

/// The shipped connector, loaded from `providers/algolia.toml`.
fn algolia() -> Connector {
    let path = providers_dir().join("algolia.toml");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    provider::load(&path.to_string_lossy(), &source)
        .unwrap_or_else(|error| panic!("providers/algolia.toml must load:\n{error}"))
        .connector
}

/// **The acceptance assertion, and the whole of C-229: one question, one answer, two positions.**
///
/// The application id composes the hostname *and* travels as `X-Algolia-Application-Id` on every
/// call. That is **one** `[[config]]` field with one `name`, one `label`, one `help` and one
/// host-side slot — not two fields an operator answers with the same string twice, which is the
/// shape C-164 weighed and refused to ship.
///
/// The three facts derivation is responsible for come along unchanged: the field is connection
/// level, it is not a secret, and it is required.
#[test]
fn the_application_id_is_one_question_reaching_two_positions() {
    let connector = algolia();

    let app_id = connector
        .config_field("app_id")
        .expect("`[[config]]` must ask for the application id this connection uses");

    assert_eq!(
        app_id.bindings(),
        Some(vec![
            Binding::Endpoint { variable: "app_id" },
            Binding::Request {
                position: Position::Header,
                name: "X-Algolia-Application-Id",
            },
        ]),
        "the hostname and the header are the two destinations of one declared value"
    );

    // **One host-side slot.** A host keys a configuration value by `(tenant, provider, service,
    // kind, name)`, so this is the property that makes it one question rather than two wearing one
    // label — and it is `binds`' own target, whatever else the field reaches.
    assert_eq!(app_id.slot(), Some("app_id"));

    // Nothing else asks for the application id. A second field would be the two-slot shape C-164
    // measured, and it would pass every other assertion here.
    let asking: Vec<&str> = connector
        .config
        .iter()
        .filter(|field| {
            field.binds.contains("app_id")
                || field
                    .also_binds
                    .iter()
                    .any(|binds| binds.contains("Application-Id"))
        })
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(
        asking,
        ["app_id"],
        "one value, one question — a second field for the same answer is the defect this connector \
         was blocked on"
    );

    assert_eq!(
        app_id.level(),
        Some(Level::Connection),
        "an application id is one per tenant, not one per vendor"
    );
    assert!(
        !app_id.secret,
        "Algolia publishes the application id in client-side code; marking it secret would hide it \
         from the operator who has to read it back, and claim gating this repository does not \
         provide"
    );
    assert!(
        app_id.required,
        "a host refuses the whole request when a pinned value is missing, so an optional pin is a \
         connector that composes no URL"
    );

    // And the API key remains a credential, which is the distinction the whole story rests on.
    let key = connector.config_field("api_key").expect("declared");
    assert!(key.secret);
    assert_eq!(
        key.binding(),
        Some(Binding::Credential {
            name: "algolia.api_key"
        })
    );
    assert!(
        key.also_binds.is_empty(),
        "a credential resolves through the secret port and cannot share a placeholder with anything"
    );
}

/// **Which placeholder the emitted module carries when the two destinations spell the value
/// differently** — the design interaction C-229 named as the one most likely to be discovered late,
/// settled here against the artifact rather than against prose.
///
/// `Position`'s `name` is deliberately both the placeholder and the wire spelling, and this field
/// breaks the coincidence: `app_id` in the host, `X-Algolia-Application-Id` on the wire. The answer
/// is that the emitted module carries **`binds`' own target everywhere** — `{app_id}` in the base
/// URL literal *and* in the literal the header record reads — so a host resolves one variable and
/// substitutes it into both positions. The header's name is only what the vendor sees.
#[test]
fn the_two_destinations_carry_one_placeholder_into_the_emitted_module() {
    let connector = algolia();
    let app_id = connector.config_field("app_id").expect("declared");

    assert_eq!(
        app_id.pins(),
        vec![Pin {
            position: Position::Header,
            name: "X-Algolia-Application-Id",
            variable: "app_id",
        }],
        "the pin's wire name is the vendor's; the placeholder it carries is the field's slot"
    );

    for operation in &connector.operations {
        let flux = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` must emit: {error}", operation.id));

        assert!(
            flux.contains(r#"base = "https://{app_id}.algolia.net""#),
            "`{}` must compose its host from the pinned placeholder:\n{flux}",
            operation.id
        );
        assert!(
            flux.contains(r#"X_Algolia_Application_Id = "{app_id}""#),
            "`{}` must bind the header to a literal carrying the **slot's** placeholder — a pin \
             carrying its own spelling would ask a host for a value nobody was asked to \
             supply:\n{flux}",
            operation.id
        );
        assert!(
            flux.contains(r#""X-Algolia-Application-Id": X_Algolia_Application_Id"#),
            "`{}` must send the pinned symbol as the header Algolia requires:\n{flux}",
            operation.id
        );
        assert!(
            !flux.contains("{X-Algolia-Application-Id}"),
            "`{}` carries a second placeholder for one value, which is the two-slot shape this \
             connector exists not to have:\n{flux}",
            operation.id
        );
        assert!(
            !flux.contains("app_id:"),
            "`{}` declares the application id as a caller argument; a pinned value a model can pass \
             is not pinned:\n{flux}",
            operation.id
        );
    }
}

/// The curated surface, and the two declarations that carry a judgement rather than a transcription.
///
/// The delete is `destructive` and is **not** claimed idempotent: Algolia accepts a delete for an id
/// that does not exist and answers with a fresh `taskID` rather than repeating the first response,
/// and documents no idempotency guarantee. The save is a `PUT` whose body is the record's whole
/// content, so it is genuinely safe to repeat — declared `conditional` with the condition stated,
/// rather than flat `idempotent`, because the write is asynchronous and a stored result must never
/// be served in place of running it.
#[test]
fn the_curated_surface_declares_its_writes_honestly() {
    let connector = algolia();

    let ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "algolia-index-list",
            "algolia-index-search",
            "algolia-object-get",
            "algolia-object-save",
            "algolia-object-delete",
        ],
        "the curated set C-164 names: list indices, search one, and get/save/delete one record"
    );

    let delete = connector
        .operation("algolia-object-delete")
        .expect("declared");
    assert_eq!(delete.risk, Risk::Destructive);
    assert!(
        delete.repeatable_because.is_none(),
        "no repeat guarantee is documented, so none is claimed"
    );

    let save = connector
        .operation("algolia-object-save")
        .expect("declared");
    assert!(
        save.repeatable_because.is_some(),
        "a `conditional` claim is worth nothing unless the condition is stated"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("algolia-index-list"),
        "the connection test must be a read that runs unattended, and it must exercise the pair \
         this connector's configuration is about"
    );
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

/// **The one route that reaches a header through `[[auth]]` forces `secret = true`, and the
/// application id is not a secret.**
///
/// Binding the application id to a credential (`Binding::Credential`, whose `is_secret()` is always
/// `true`) is how this model placed a value into an arbitrary request header before C-187. The
/// loader enforces the agreement unconditionally: a `[[config]]` field binding a credential while
/// declaring `secret = false` is refused, naming exactly this contradiction. Declaring `secret =
/// true` instead would be the dishonest fix the story explicitly rules out — the application id is
/// meant to be readable back, logged, and shown in a UI, none of which a secret field permits.
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

/// **Two fields under one name are still refused as a shared slot** — C-164's tripwire, kept.
///
/// This is the rule C-229 had to preserve, and the one it would have been easiest to widen into a
/// hole. If two `[[config]]` fields spell the `base_url` placeholder and the header with the *same*
/// name, they resolve one `{placeholder}` and a host keys both to one value under one slot — the
/// C-197 collapse, where one field's answer is silently discarded.
///
/// **Declaring one question with two destinations is a different statement**, and the difference is
/// exactly what makes one legal and the other not: `also_binds` keeps one field, one `name`, one
/// slot and one answer, where this shape has two of each but one place to put them. The refusal
/// still fires, still names both fields, and now names the declaration that would have been right.
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

    // And the same two destinations, declared as **one** question, load — which is the whole of
    // C-229 stated as the contrast the refusal above exists against.
    let one_question = r#"
[[config]]
name = "app_id"
label = "Algolia application id"
help = "For the probe fixture only"
binds = "endpoint.app_id"
also_binds = ["header.X-Algolia-Application-Id"]
"#;
    provider::load("providers/algolia.toml", &fixture("", one_question))
        .expect("one field, one name, one slot, two destinations");
}

/// **A header pin does not satisfy a `base_url` template variable**, so the header alone cannot
/// stand in for the hostname either — C-164's second tripwire, kept, and the reason the `endpoint.`
/// destination is `binds` rather than an `also_binds` entry.
///
/// `validate_every_template_variable_is_asked_for` matches only `Binding::Endpoint`, so a
/// `{placeholder}` in a `base_url` is bound by an endpoint binding or by nothing. C-229 does not
/// move that: `also_binds` accepts request positions only, and the field that binds Algolia's
/// hostname *and* its header binds the hostname in `binds` — which is what makes `{app_id}` the one
/// placeholder both destinations carry.
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

    // The same asymmetry from the other side: an `endpoint.` destination in `also_binds` is refused
    // outright, because the emitted module carries `binds`' target and a second `base_url` variable
    // would be filled from a slot that is not its own — and reach the vendor as text.
    let inverted = r#"
[[config]]
name = "application_id"
label = "Algolia application id"
help = "For the probe fixture only"
binds = "header.X-Algolia-Application-Id"
also_binds = ["endpoint.app_id"]
"#;
    let error = provider::load("providers/algolia.toml", &fixture("", inverted))
        .expect_err("a `base_url` variable belongs in `binds`");
    assert!(
        error.to_string().contains("also_binds"),
        "expected the refusal to name the key that was misused, got: {error}"
    );
}
