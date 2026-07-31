//! Mailchimp (C-215) is the epic's probe for **a host component an operator supplies that the
//! vendor also hides inside the credential**, and for **a Basic mechanism whose username is a
//! constant rather than operator-supplied**. Both are measured here rather than described.
//!
//! Mailchimp's root URL is `https://<dc>.api.mailchimp.com/3.0`, and the vendor's own Fundamentals
//! page (read 2026-07-31) finds `<dc>` three ways: *"It's the first part of the URL you see in the
//! API keys section of your account"*, *"It's also appended to your API key in the form key-dc"*,
//! and — on an OAuth 2 connection — the OAuth metadata endpoint. The second route is the one this
//! connector must not take, and the reason is one line: composing a **host** out of a **secret**
//! would put a substring of the
//! credential into every URL, log line and error message the connection produces, while the value
//! the host registered with its redactor is the whole key — so the piece that travels in the clear
//! is a piece nothing scrubs, and it is a piece an attacker can use to narrow the rest.
//!
//! Four findings, each pinned below:
//!
//! 1. **`{dc}` is asked for, and it is structurally incapable of being a credential.** It is a
//!    `[[config]]` field binding `endpoint.dc`, which `Binding::is_secret` answers `false` for
//!    unconditionally — so the loader refuses the `secret = true` spelling, and no rewrite of this
//!    file can turn the datacenter into something the secret store holds.
//! 2. **Nothing in the emitted module reads the credential.** Every operation's URL is composed from
//!    the template alone; the credential's name and its environment variable appear nowhere in the
//!    emitted Flux.
//! 3. **A constant Basic username is not expressible today, so this connector is not Basic.** The
//!    loader refuses `scheme = "basic"` without `user_env`, and a declared `user_env` is resolved
//!    **per tenant from the configuration port** (`connector-pack`'s `user_half`, C-193) and is
//!    mandatory — so the Basic spelling would ask every operator to type a string Mailchimp
//!    documents as arbitrary. Mailchimp publishes `Authorization: Bearer <TOKEN>` as an equal
//!    alternative, so this connector takes it and asks for nothing it cannot use.
//! 4. **Nothing reaches a query string.** The emitter interpolates query values verbatim
//!    (`crates/connector-flux/src/op.rs`; `AGENTS.md`'s `zendesk-ticket-search` gap), so this first
//!    curated set declares no query parameter at all — which for Mailchimp means every one of its
//!    `fields`, `count`, `offset` and `status` filters is deliberately absent, named in the provider
//!    file rather than silently missing.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{
    provider, AuthScheme, Binding, Connector, Format, HttpMethod, Idempotency, Level, Risk,
};

/// The provider under test.
const PROVIDER: &str = "mailchimp";

/// The sole credential. A *name*; no credential value appears in this repository.
const CREDENTIAL: &str = "mailchimp.api_key";
/// A variable *name*, not a value.
const CREDENTIAL_ENV: &str = "MAILCHIMP_API_KEY";

/// The templated root URL. `{dc}` is the probe.
const BASE_URL: &str = "https://{dc}.api.mailchimp.com/3.0";
/// The template variable an operator fills in.
const DC_VARIABLE: &str = "dc";
/// The `[[config]]` field that asks for it.
const DC_FIELD: &str = "server_prefix";

/// The verification read — argument-free, so a settings page can run it unattended.
const VERIFY: &str = "mailchimp-ping";

/// The curated set, in the order `providers/mailchimp.toml` declares it, with the effects each one
/// claims. Six reads and one write.
const OPERATIONS: &[(&str, HttpMethod, Risk, Idempotency)] = &[
    (
        "mailchimp-ping",
        HttpMethod::Get,
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "mailchimp-audience-list",
        HttpMethod::Get,
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "mailchimp-audience-get",
        HttpMethod::Get,
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "mailchimp-audience-member-list",
        HttpMethod::Get,
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "mailchimp-audience-member-get",
        HttpMethod::Get,
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "mailchimp-audience-member-upsert",
        HttpMethod::Put,
        Risk::High,
        Idempotency::Idempotent,
    ),
    (
        "mailchimp-campaign-list",
        HttpMethod::Get,
        Risk::Low,
        Idempotency::Idempotent,
    ),
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn source() -> String {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-215 ships the Mailchimp connector",
            path.display()
        )
    })
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    provider::load(&format!("providers/{PROVIDER}.toml"), &source())
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// **Finding 1: the datacenter is configuration, and it cannot be anything else.**
///
/// `{dc}` is the one template variable in the base URL, exactly one field binds it, and that field
/// is non-secret connection-level configuration with a renderable label, help text and a vendor
/// docs link. The negative half is what makes it a probe rather than a description: a field binding
/// `endpoint.*` is `is_secret() == false` unconditionally
/// (`crates/connector-spec/src/config.rs`, `Binding::is_secret`), so the loader refuses the
/// `secret = true` spelling of this same field. There is therefore no edit to this provider file
/// that routes the datacenter through the secret store — the classification is structural.
#[test]
fn the_datacenter_is_asked_for_as_ordinary_configuration() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Mailchimp");
    assert_eq!(connector.base_url, BASE_URL);
    assert_eq!(connector.authority.as_deref(), Some("com.mailchimp.api"));
    assert_eq!(connector.api_version.as_deref(), Some("3.0"));

    let variables = connector_spec::config::template_variables(&connector.base_url);
    assert_eq!(
        variables,
        [DC_VARIABLE],
        "the datacenter is the only thing an operator has to fill into the host"
    );

    let binding: Vec<&str> = connector
        .config
        .iter()
        .filter(|field| {
            matches!(field.binding(), Some(Binding::Endpoint { variable }) if variable == DC_VARIABLE)
        })
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(
        binding,
        [DC_FIELD],
        "exactly one field asks for `{{{DC_VARIABLE}}}` — two would be one question with two answers"
    );

    let field = connector
        .config
        .iter()
        .find(|field| field.name == DC_FIELD)
        .expect("the datacenter field is declared");
    assert!(
        !field.secret,
        "the datacenter is part of a hostname; a secret must never be one"
    );
    assert_eq!(field.level(), Some(Level::Connection));
    assert_eq!(
        field.format,
        Format::Subdomain,
        "`us14` is one DNS label, and a renderer that knows that can explain a rejection"
    );
    assert!(field.required, "no request has a destination without it");
    assert!(!field.label.is_empty() && !field.help.is_empty());
    assert!(
        field.help.contains('-'),
        "the help text tells an operator where to find the value — including that it is the part of \
         their key after the dash, which is what makes reading it out of the credential unnecessary"
    );
    assert!(
        field.docs_url.is_some(),
        "the vendor's own page, not ours, is where an operator looks it up"
    );

    // The refusal, measured rather than described: the secret spelling of this field does not load.
    let secret_datacenter = source().replace(
        "name = \"server_prefix\"",
        "name = \"server_prefix\"\nsecret = true",
    );
    assert_ne!(
        secret_datacenter,
        source(),
        "the substitution must actually apply, or the refusal below proves nothing"
    );
    let error = provider::load(&format!("providers/{PROVIDER}.toml"), &secret_datacenter)
        .expect_err("a field binding a base-URL template variable cannot declare `secret = true`");
    let message = error.to_string();
    assert!(
        message.contains(DC_FIELD) && message.contains("secret"),
        "expected the secret/binds agreement refusal naming the field, got: {message}"
    );
}

/// **Finding 2: no emitted operation reads the credential to compose its host.**
///
/// The tempting shortcut this story exists to refuse is deriving `{dc}` from the API key's `-us14`
/// suffix. It is refused at the declaration by finding 1; this is the other end of the pipeline. The
/// emitted module composes every URL from the base template alone, and neither the credential's name
/// nor its environment variable appears anywhere in the Flux — the host resolves and places the
/// secret, and the module never holds it.
#[test]
fn no_emitted_operation_composes_its_host_from_the_credential() {
    let connector = load();

    for operation in &connector.operations {
        let flux = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{} does not emit: {error}", operation.id));
        assert!(
            flux.contains(&format!("base = \"{BASE_URL}\"")),
            "{} does not carry the templated host verbatim:\n{flux}",
            operation.id
        );
        assert!(
            !flux.contains(CREDENTIAL) && !flux.contains(CREDENTIAL_ENV),
            "{} names the credential in emitted Flux. A host composed from a secret puts a \
             substring of that secret into every URL a redactor holding the whole key would not \
             scrub:\n{flux}",
            operation.id
        );
    }
}

/// **Finding 3: a constant Basic username is not expressible, so this connector is a bearer token.**
///
/// Mailchimp's documented Basic form is `--user 'anystring:<TOKEN>'`: the username is a literal the
/// vendor ignores. This repository cannot say that. `AuthScheme::Basic` composes
/// `base64(<user><user_suffix>:<secret>)` where `<user>` comes from `user_env`, the loader refuses a
/// `basic` credential that declares none, and `user_suffix` *appends to* a resolved value rather
/// than replacing it. Worse, the resolved half is not a deployment constant either: `connector-pack`
/// reads it from the **configuration port, per tenant**, and refuses the request when it is absent
/// (C-193, `crates/connector-pack/src/credentials.rs`), so the Basic spelling would put a mandatory
/// "type the word anystring" field in front of every operator connecting an account.
///
/// The vendor makes the choice cheap: *"API keys and OAuth 2 tokens can be used to make
/// authenticated requests the same way"*, with `Authorization: Bearer <TOKEN>` shown beside the
/// Basic form on the same page. So this connector ships bearer, asks for one value, and the gap is
/// recorded rather than worked around. Both halves are asserted — what ships, and that the
/// alternative genuinely does not load.
#[test]
fn the_credential_is_a_bearer_token_because_a_constant_username_cannot_be_declared() {
    let connector = load();

    assert_eq!(
        connector.auth.len(),
        1,
        "one credential: the API key, which is also the whole of Mailchimp's key-based auth"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("mailchimp declares `{CREDENTIAL}`"));
    assert_eq!(method.scheme, AuthScheme::Bearer);
    assert_eq!(method.env, [CREDENTIAL_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a bearer has no user half"
    );

    assert!(
        !connector
            .config
            .iter()
            .any(|field| matches!(field.binding(), Some(Binding::Username { .. }))),
        "no operator is asked for a username. That is the whole finding: the only spelling of \
         Mailchimp's `anystring` this IR admits is a mandatory per-tenant question with an \
         arbitrary answer"
    );

    let mechanisms: Vec<Vec<&str>> = connector
        .default_auth
        .iter()
        .map(|requirement| requirement.iter().map(String::as_str).collect())
        .collect();
    assert_eq!(
        mechanisms,
        vec![vec![CREDENTIAL]],
        "one mechanism, one credential — Mailchimp's OAuth 2 access token travels in the same \
         header and is not a second declaration"
    );
    for (id, ..) in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("mailchimp declares `{id}`"));
        let effective: Vec<Vec<&str>> = connector
            .effective_auth(operation)
            .iter()
            .map(|requirement| requirement.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(
            effective,
            vec![vec![CREDENTIAL]],
            "{id} authenticates like every other operation; even `/ping` needs the key"
        );
    }

    // The refusal, measured: the Basic spelling of a constant username — a `basic` credential with
    // no `user_env`, because there is no env variable holding a literal the vendor ignores — does
    // not load, and the message says what is missing.
    let constant_username = source().replace("scheme = \"bearer\"", "scheme = \"basic\"");
    assert_ne!(
        constant_username,
        source(),
        "the substitution must actually apply, or the refusal below proves nothing"
    );
    let error = provider::load(&format!("providers/{PROVIDER}.toml"), &constant_username)
        .expect_err("a `basic` credential declaring no `user_env` is refused");
    let message = error.to_string();
    assert!(
        message.contains("user_env"),
        "expected the missing-user-half refusal, got: {message}"
    );
}

/// **Finding 4: the curated set, its declared effects, and an empty query string.**
///
/// Every Mailchimp collection endpoint documents `fields`, `exclude_fields`, `count` and `offset`,
/// and the members list adds `status`, `vip_only` and six timestamp filters. None is declared here:
/// a query value is interpolated verbatim by the emitter, so a caller's `&` reshapes the request
/// rather than filtering it. The provider file names each excluded parameter instead of leaving it
/// silently absent, and this test pins the consequence — no operation declares a query parameter and
/// no emitted URL contains a `?`.
#[test]
fn the_curated_set_declares_its_effects_and_puts_nothing_in_a_query_string() {
    let connector = load();

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    let expected: Vec<&str> = OPERATIONS.iter().map(|(id, ..)| *id).collect();
    assert_eq!(declared, expected, "the curated set, in declaration order");

    for (id, method, risk, idempotency) in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("mailchimp declares `{id}`"));
        assert_eq!(operation.method, *method, "{id}");
        assert_eq!(operation.risk, *risk, "{id}");
        assert_eq!(operation.idempotency, *idempotency, "{id}");
        assert!(
            !operation.description.is_empty(),
            "{id} carries the contract a model reads"
        );
        assert!(
            operation.params.query.is_empty(),
            "{id} declares a query parameter, and every query value this emitter writes is \
             interpolated verbatim"
        );

        let flux = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{id} does not emit: {error}"));
        assert!(
            !flux.contains('?'),
            "{id} emits a `?` into its URL:\n{flux}"
        );
    }

    // The one write, stated positively: the free text it carries is a body, and it is the only
    // operation in the set that is not a read.
    let upsert = connector
        .operation("mailchimp-audience-member-upsert")
        .expect("the curated set includes the contact write");
    let body: Vec<&str> = upsert
        .params
        .body
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    assert_eq!(
        body,
        ["email_address", "status_if_new", "status"],
        "the address and its consent status travel in a JSON body"
    );
    let flux = emit_operation(&connector, upsert).expect("the contact write emits");
    assert!(
        flux.contains("content_type = \"application/json\""),
        "the contact write sends a JSON body:\n{flux}"
    );
    assert_eq!(
        connector
            .operations
            .iter()
            .filter(|operation| operation.method != HttpMethod::Get)
            .count(),
        1,
        "one write in a set of seven; everything else Mailchimp offers is excluded and named"
    );
}

/// **`verify` is a read that runs unattended.**
///
/// A "Test connection" button is pressed whenever someone opens a settings page, so it must be a
/// read (the loader checks the declared risk) *and* it must need no argument, which the loader does
/// not check and a connector can still get wrong. `GET /ping` proves both halves of this
/// connector's configuration at once — a wrong key and a wrong datacentre label both fail it — and
/// Mailchimp describes it as *"A health check for the API that won't return any account-specific
/// information"*, which is less than the API root the vendor's own quick start calls and less than
/// an unattended button should pull. `providers/mailchimp.toml` records why the root is excluded.
#[test]
fn verify_is_an_argument_free_read() {
    let connector = load();

    assert_eq!(connector.verify.as_deref(), Some(VERIFY));
    let operation = connector
        .operation(VERIFY)
        .expect("verify names an operation");

    assert_eq!(operation.method, HttpMethod::Get);
    assert_eq!(operation.risk, Risk::Low);
    assert_eq!(operation.idempotency, Idempotency::Idempotent);
    assert!(
        operation.params.path.is_empty()
            && operation.params.query.is_empty()
            && operation.params.body.is_empty()
            && operation.params.header.is_empty()
            && operation.params.body_schema.is_none(),
        "a connection test that needs an argument cannot run unattended"
    );
}
