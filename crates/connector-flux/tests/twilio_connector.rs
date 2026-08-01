//! The Twilio connector, and the two decisions that shaped it — C-109.
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims specific to Twilio, because they are the reasons the
//! connector looks the way it does and the reasons a later reader must not "modernise" the file:
//!
//! - **No operation sends a body.** Every Twilio write is `application/x-www-form-urlencoded`, and
//!   `params.body_encoding = "form"` (C-144) interpolates form values verbatim — flux has no form
//!   encoder yet, because the encoder that exists upstream (flux's `L-101`) is not in the pinned
//!   `codewandler-flux-lang` release. A value carrying `&` or `=` would corrupt the body. So this
//!   connector ships reads only, over Basic auth, until the encoder publishes.
//! - **The Account SID is both this connector's Basic-auth username and a required path parameter
//!   on every operation.** `ConfigField::binds` admits exactly one destination per field
//!   (`crates/connector-spec/src/config.rs:239-267`), so the one visible `[[config]]` field binds to
//!   `username.twilio.basic_auth` and the SID is *not* templated into `base_url` — it travels as a
//!   real, caller-facing `account_sid` path parameter instead. [`every_twilio_operation_requires_the_account_sid_in_its_path`]
//!   is what stops that duplication from being "cleaned up" back into a second config field or a
//!   silently-invented `base_url` template.
//! - **No query parameter carries a phone number or a range operator.** `To`, `From` and Twilio's
//!   `DateSent`/`StartTime` range filters are all documented, and all excluded: an unencoded `+` in a
//!   phone number, or `<`/`>` in a parameter *name*, is exactly the class of defect
//!   `zendesk-ticket-search` (C-29) carries. [`no_twilio_query_parameter_is_unshippable`] pins the
//!   safe subset that is declared instead.
//! - **No `[[channels]]` webhook binding**, even though `[[events]]` are declared. Twilio signs a
//!   status callback over the request URL plus its sorted, reassembled form fields — not a template
//!   over `{body}`/`{timestamp}`, which is all `HmacSpec::signed` accepts — so verification cannot be
//!   declared honestly yet. [`twilio_declares_no_channel_binding_for_its_events`] is the same shape of
//!   assertion `stripe_connector.rs` makes for `Stripe-Signature`.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider under test.
const PROVIDER: &str = "twilio";

/// The Basic credential every operation authenticates with.
const CREDENTIAL: &str = "twilio.basic_auth";
/// The secret half's environment variable. A *name*; no credential value appears in this repository.
const AUTH_TOKEN_ENV: &str = "TWILIO_AUTH_TOKEN";
/// The non-secret identity half: the Account SID.
const ACCOUNT_SID_ENV: &str = "TWILIO_ACCOUNT_SID";
/// The `signing`-scheme credential declared for status-callback verification, unreferenced by any
/// channel today — see the module docs.
const SIGNING_CREDENTIAL: &str = "twilio.webhook_signing_secret";

/// The curated operations, in the order `providers/twilio.toml` declares them. All five are reads.
const OPERATIONS: &[&str] = &[
    "twilio-account-get",
    "twilio-message-list",
    "twilio-message-get",
    "twilio-call-list",
    "twilio-call-get",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// Every operation's emitted module text, paired with its operation id.
fn emitted(connector: &Connector) -> Vec<(String, String)> {
    connector
        .operations
        .iter()
        .map(|operation| {
            let text = emit_operation(connector, operation)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
            (operation.id.clone(), text)
        })
        .collect()
}

/// The connector exists, loads, and is the one the story specifies: Basic auth over a fixed host,
/// with the Account SID as the username half rather than an email.
#[test]
fn the_twilio_connector_loads_and_authenticates_with_an_account_sid_and_auth_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Twilio");
    assert_eq!(
        connector.base_url, "https://api.twilio.com/2010-04-01",
        "Twilio does not multi-tenant by host, unlike zendesk/freshdesk/jira; base_url carries no \
         {{placeholder}}"
    );

    assert_eq!(
        connector.auth.len(),
        2,
        "one basic credential for outbound requests, plus the signing credential declared for \
         status-callback verification (unreferenced until a channel binding can express Twilio's \
         signature scheme)"
    );

    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("twilio declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Basic,
        "Twilio's Account SID / Auth Token pair is HTTP Basic"
    );
    assert_eq!(method.env, [AUTH_TOKEN_ENV]);
    assert_eq!(method.user_env, [ACCOUNT_SID_ENV]);
    assert!(
        method.user_suffix.is_none(),
        "twilio's user half is the bare Account SID; no vendor marker is appended"
    );

    let signing = connector
        .auth_method(SIGNING_CREDENTIAL)
        .unwrap_or_else(|| panic!("twilio declares `{SIGNING_CREDENTIAL}`"));
    assert_eq!(signing.scheme, AuthScheme::Signing);
    // Twilio issues exactly one secret; the signing credential resolves the same environment
    // variable as the Basic credential's secret half, deliberately (see the provider file's comment).
    assert_eq!(signing.env, [AUTH_TOKEN_ENV]);

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; twilio is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the Basic pair",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares caller-supplied headers; the Authorization header is injected \
             by the host",
            operation.id
        );
    }

    assert_eq!(
        connector.verify.as_deref(),
        Some("twilio-account-get"),
        "verify must be a read; the account fetch is bounded to the caller's own account"
    );
}

/// **The Account SID is a required path parameter on every operation — not a `base_url` template —
/// and every operation's path is account-scoped.**
///
/// This is the load-bearing assertion for C-109's second finding: `ConfigField::binds` cannot send
/// one collected value to both `endpoint.account_sid` and `username.twilio.basic_auth`, so the SID
/// is authored once in config (as the username) and again as a real path parameter every operation
/// declares. A later author "fixing" the duplication by templating `{account_sid}` into `base_url`
/// would silently drop this parameter and break every emitted request.
#[test]
fn every_twilio_operation_requires_the_account_sid_in_its_path() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.path.starts_with("/Accounts/{account_sid}"),
            "operation `{}` has path `{}`; every operation is scoped under the caller's account",
            operation.id,
            operation.path
        );

        let account_sid = operation
            .params
            .path
            .iter()
            .find(|param| param.name == "account_sid")
            .unwrap_or_else(|| {
                panic!(
                    "operation `{}` declares no `account_sid` path param",
                    operation.id
                )
            });
        assert!(
            account_sid.required,
            "operation `{}`'s `account_sid` must be required; there is no fallback value",
            operation.id
        );
    }

    for (id, path) in [
        ("twilio-account-get", "/Accounts/{account_sid}.json"),
        (
            "twilio-message-list",
            "/Accounts/{account_sid}/Messages.json",
        ),
        (
            "twilio-message-get",
            "/Accounts/{account_sid}/Messages/{message_sid}.json",
        ),
        ("twilio-call-list", "/Accounts/{account_sid}/Calls.json"),
        (
            "twilio-call-get",
            "/Accounts/{account_sid}/Calls/{call_sid}.json",
        ),
    ] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("twilio declares `{id}`"));
        assert_eq!(operation.path, path, "`{id}` addresses the wrong resource");
    }
}

/// **No query parameter carries a phone number or a range-operator name.**
///
/// `To`, `From`, and Twilio's `DateSent`/`StartTime`/`EndTime` range filters (`DateSent<`,
/// `DateSent>`, ...) are documented Twilio list filters and are all absent: a phone number's leading
/// `+` and a range operator living in the parameter *name* are both unencoded by this emitter's query
/// assembly (C-29). Only `Page`, `PageSize` and `Status` — plain integers and an enum word — are
/// declared.
#[test]
fn no_twilio_query_parameter_is_unshippable() {
    let connector = load();

    let unshippable = ["To", "From", "DateSent", "StartTime", "EndTime"];
    for operation in &connector.operations {
        for param in &operation.params.query {
            let wire = param.wire.as_deref().unwrap_or(param.name.as_str());
            assert!(
                !unshippable.iter().any(|bad| wire.starts_with(bad)),
                "operation `{}` declares query parameter `{}`, which is unshippable: it either \
                 carries an E.164 phone number (unencoded `+`) or a range operator in its own name \
                 (unencoded `<`/`>`)",
                operation.id,
                wire
            );
            assert!(
                !wire.contains('<') && !wire.contains('>'),
                "operation `{}` declares query parameter `{}` carrying a raw range operator",
                operation.id,
                wire
            );
        }
    }

    let call_list = connector
        .operations
        .iter()
        .find(|operation| operation.id == "twilio-call-list")
        .expect("twilio declares `twilio-call-list`");
    let declared: Vec<&str> = call_list
        .params
        .query
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    assert_eq!(declared, ["status", "page", "page_size"]);
}

/// No operation declares a body field of any kind — the connector ships reads only, and the header
/// comment records why (C-144, unpublished flux form encoder).
#[test]
fn no_twilio_operation_declares_a_body() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.params.body.is_empty(),
            "operation `{}` declares a body field; twilio ships no writes until the form encoder \
             publishes (C-144)",
            operation.id
        );
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form body_schema; twilio ships no writes",
            operation.id
        );
        assert_eq!(
            operation.method,
            HttpMethod::Get,
            "operation `{}` uses a method this connector does not curate",
            operation.id
        );
        assert_eq!(
            operation.risk,
            Risk::Low,
            "operation `{}` is a read",
            operation.id
        );
        assert_eq!(
            operation.idempotency,
            Idempotency::Idempotent,
            "operation `{}` is a GET, which is repeatable",
            operation.id
        );
    }
}

/// `[[events]]` are declared for status callbacks, but `[[channels]]` is empty.
///
/// The same shape of assertion `stripe_connector.rs` makes for `Stripe-Signature`: Twilio signs a
/// status callback over the request URL plus its sorted, reassembled form fields, which is not a
/// template over `{body}`/`{timestamp}` — the only two placeholders `HmacSpec::signed` accepts — so
/// no `[[channels]]` binding can verify it honestly yet.
#[test]
fn twilio_declares_no_channel_binding_for_its_events() {
    let connector = load();

    let event_names: Vec<&str> = connector
        .events
        .iter()
        .map(|event| event.name.as_str())
        .collect();
    assert_eq!(
        event_names,
        ["message.status_callback", "call.status_callback"]
    );

    assert!(
        connector.channels.is_empty(),
        "twilio declares a `[[channels]]` binding, but its signature scheme (URL + sorted form \
         fields) cannot be expressed by `HmacSpec` — see the provider file's block comment"
    );
}

/// The configuration surface asks for the Account SID once — bound to the credential's username,
/// not templated into `base_url` — and the Auth Token as the gated secret.
#[test]
fn the_config_surface_asks_for_the_account_sid_exactly_once() {
    let connector = load();

    assert_eq!(
        connector.config.len(),
        2,
        "exactly two fields: the Account SID and the Auth Token. No third field re-asks for the SID"
    );

    let sid_field = connector
        .config
        .iter()
        .find(|field| field.name == "account_sid")
        .expect("twilio declares an `account_sid` config field");
    assert_eq!(sid_field.binds, "username.twilio.basic_auth");
    assert!(
        !sid_field.secret,
        "the Account SID is the non-secret username half"
    );

    let token_field = connector
        .config
        .iter()
        .find(|field| field.name == "auth_token")
        .expect("twilio declares an `auth_token` config field");
    assert_eq!(token_field.binds, "credential.twilio.basic_auth");
    assert!(
        token_field.secret,
        "the Auth Token must be gated as a secret"
    );
    assert!(
        token_field.example.is_none(),
        "a secret field must carry no realistic-looking example"
    );
}

/// **No credential reaches a generated module** — not a value, and not even a variable name.
#[test]
fn no_twilio_module_carries_a_credential_or_its_variable_name() {
    let connector = load();

    for (id, text) in emitted(&connector) {
        for forbidden in [
            AUTH_TOKEN_ENV,
            ACCOUNT_SID_ENV,
            CREDENTIAL,
            SIGNING_CREDENTIAL,
            "$secret",
            "Authorization",
        ] {
            assert!(
                !text.contains(forbidden),
                "`{id}` names `{forbidden}` in generated Flux; a generated module carries no \
                 credential and no credential reference (C-10, AGENTS.md):\n{text}"
            );
        }
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical,
/// and **loads** as exactly one exposed composite op.
#[test]
fn every_twilio_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "`{}` emits Flux that does not parse: {:?}\n{emitted}",
            operation.id,
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str()),
            "the flux formatter would rewrite `{}`",
            operation.id
        );

        let module = flux_lang::program::Module::parse_str(&emitted)
            .unwrap_or_else(|error| panic!("`{}` does not load: {error}", operation.id));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{}` is not a program", operation.id));
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, operation.id);
        assert!(
            program.ops[0].meta.expose,
            "`{}` must be exposed to the model as a tool",
            operation.id
        );
    }
}
