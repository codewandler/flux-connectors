//! The PagerDuty connector, and the three decisions that shaped it — C-162.
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims specific to PagerDuty, because they are the reasons the
//! connector looks the way it does and the reasons a later reader must not "modernise" the file:
//!
//! - **`Authorization: Token token=<key>` is a *prefix*, and nothing more.** The story that filed
//!   this connector called the credential a "substructure, not a suffix" of the header value, and
//!   that framing is dead: [C-161](../../../docs/stories/C-161-provider-okta.md) measured three
//!   vendors and found one shape, and [C-184](../../../docs/stories/C-184-auth-scheme-prefix-axis.md)
//!   built the axis. The credential **ends** the header value here, so `prefix = "Token token="` is
//!   complete — the `=` is a character in a prefix and not a fourth axis. There is deliberately no
//!   `suffix` and no value template (`docs/designs/unified-auth.md` §"The prefix axis, as built"),
//!   and [`the_pagerduty_credential_is_a_header_prefix_and_the_credential_ends_the_value`] is what
//!   stops a later author reintroducing one for this vendor.
//! - **`From` is a required *operation parameter*, not configuration.** PagerDuty requires a `From`
//!   header carrying the acting user's email on an incident write. It is not a credential, and it
//!   cannot be configuration either: `parse_binding` (`crates/connector-spec/src/config.rs:239-267`)
//!   admits exactly five destinations — `endpoint.*`, `credential.*`, `username.*`,
//!   `oauth.client_id`, `oauth.client_secret` — and **there is no `header.*` binding**, so an
//!   operator-configured `From` is unspellable rather than merely unwise. It travels as a
//!   caller-facing `params.header`, the shape `providers/stripe.toml:411`'s `Idempotency-Key`
//!   already uses. [`every_pagerduty_write_requires_a_from_header_and_no_read_declares_one`] pins it.
//! - **No operation declares a pagination quirk, and that is the finding.** PagerDuty pages with
//!   `limit`/`offset`, and [`Pagination`](connector_spec::Pagination)
//!   (`crates/connector-spec/src/ir.rs:355-378`) has exactly two variants, `Page` and `Cursor`.
//!   Neither is `limit`/`offset`: `Page` describes a page *number* incremented by one, and `offset`
//!   is a row count advanced by `limit`, so `page_param = "offset"` would compile, look right, and
//!   record a claim about PagerDuty that is false. **Nothing emits a pagination loop today** — the
//!   enum is a declaration only (`crates/connector-spec/src/ir.rs:352`) and `connector-flux` reads
//!   just `quirks.error_envelope` — which is why the false declaration must be refused now rather
//!   than when C-12 compiles these into control flow and it becomes a loop re-reading all but one
//!   row of each window. So the query parameters are declared and the quirk is not, following
//!   `providers/launchdarkly.toml:123`'s precedent, and
//!   [`no_pagerduty_operation_declares_a_pagination_quirk`] exists so that absence cannot later be
//!   "fixed" into a declaration the IR cannot honestly make.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider under test.
const PROVIDER: &str = "pagerduty";

/// The one credential every operation authenticates with.
const CREDENTIAL: &str = "pagerduty.api_token";
/// Its environment variable. A *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "PAGERDUTY_API_TOKEN";

/// The header the credential is placed in, and the literal that precedes it.
const AUTH_HEADER: &str = "Authorization";
/// PagerDuty's scheme text. It ends in `=`, and the raw key follows it directly.
const AUTH_PREFIX: &str = "Token token=";

/// The curated operations, in the order `providers/pagerduty.toml` declares them.
const OPERATIONS: &[&str] = &[
    "pagerduty-incident-list",
    "pagerduty-incident-get",
    "pagerduty-service-list",
    "pagerduty-oncall-list",
    "pagerduty-incident-acknowledge",
    "pagerduty-incident-resolve",
];

/// The two writes. Everything else in [`OPERATIONS`] is a read.
const WRITES: &[&str] = &[
    "pagerduty-incident-acknowledge",
    "pagerduty-incident-resolve",
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

/// **The credential is a header prefix, and the credential ends the header value.**
///
/// This is C-162's load-bearing assertion and the reason the connector was chosen. `Token token=` is
/// a prefix that happens to contain `=`; the host appends the resolved key to it and nothing follows.
/// The assertion is written negatively as well as positively — the emitted scheme must be exactly
/// `Header { name, prefix }`, and the prefix must be the *whole* literal — because the failure mode
/// this pins out is an author reaching for a `suffix` or a `"Token token={cred}"` template, both of
/// which C-184 refused for reasons `docs/designs/unified-auth.md` records.
#[test]
fn the_pagerduty_credential_is_a_header_prefix_and_the_credential_ends_the_value() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "PagerDuty");
    assert_eq!(
        connector.base_url, "https://api.pagerduty.com",
        "PagerDuty does not multi-tenant by host; base_url carries no {{placeholder}}"
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "PagerDuty ships one credential: the REST API key. No signing secret is declared, because \
         no event or channel is"
    );

    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("pagerduty declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: AUTH_HEADER.to_string(),
            prefix: AUTH_PREFIX.to_string(),
        },
        "PagerDuty's `Authorization: Token token=<key>` is one `Header` placement with a prefix — \
         the credential ends the value, so there is nothing for a suffix or a template to carry"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "a header placement has no Basic username half"
    );

    // The prefix is authored data that reaches a header value verbatim, so it must be exactly the
    // vendor's literal — no stray whitespace, and no attempt to spell the credential inside it.
    let AuthScheme::Header { prefix, .. } = &method.scheme else {
        unreachable!("asserted immediately above")
    };
    assert!(
        prefix.ends_with('='),
        "PagerDuty's prefix ends at the `=`; the raw key follows it with no separator, so a \
         trailing space would send `Token token= <key>`"
    );
    for forbidden in [TOKEN_ENV, CREDENTIAL, "$secret", "${", "{cred}"] {
        assert!(
            !prefix.contains(forbidden),
            "the prefix spells `{forbidden}`; a prefix is a literal, and nothing interpolates it"
        );
    }

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
            "operation `{}` has {} auth alternatives; pagerduty is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the API token",
            operation.id
        );
    }

    assert_eq!(
        connector.verify.as_deref(),
        Some("pagerduty-service-list"),
        "verify must be a read that runs unattended and takes no required argument"
    );
    let verify = connector
        .operations
        .iter()
        .find(|operation| Some(operation.id.as_str()) == connector.verify.as_deref())
        .expect("verify names a declared operation");
    assert_eq!(verify.method, HttpMethod::Get);
    assert_eq!(verify.risk, Risk::Low);
    assert!(
        verify.params.iter().all(|param| !param.required),
        "verify runs unattended whenever a settings page opens; it can be given no argument"
    );
}

/// **`From` is a required header parameter on both writes, and on neither read.**
///
/// PagerDuty requires the acting user's email on an incident write. It is not a credential — it
/// authenticates nothing and gates nothing — and it cannot be operator configuration either, because
/// `ConfigField::binds` has no `header.*` destination to send a collected value to
/// (`crates/connector-spec/src/config.rs:239-267`). A caller-facing `params.header` is the one shape
/// left, and it is the shape Stripe's `Idempotency-Key` already uses.
///
/// The negative half matters as much: a read must **not** ask for it. PagerDuty does not require
/// `From` on a `GET`, and declaring it there would make every read demand a personal email address
/// for nothing.
#[test]
fn every_pagerduty_write_requires_a_from_header_and_no_read_declares_one() {
    let connector = load();

    for operation in &connector.operations {
        let from = operation
            .params
            .header
            .iter()
            .find(|param| param.wire.as_deref() == Some("From") || param.name == "from");

        if WRITES.contains(&operation.id.as_str()) {
            let from = from.unwrap_or_else(|| {
                panic!(
                    "write `{}` declares no `From` header parameter; PagerDuty rejects an incident \
                     write without one",
                    operation.id
                )
            });
            assert!(
                from.required,
                "`{}`'s `From` must be required — PagerDuty answers 400 without it",
                operation.id
            );
            assert_eq!(
                from.wire.as_deref(),
                Some("From"),
                "`{}`'s `From` must reach the wire under PagerDuty's own spelling",
                operation.id
            );
            assert_eq!(
                operation.params.header.len(),
                1,
                "`{}` declares a caller-supplied header other than `From`; the Authorization \
                 header is injected by the host and every vendor-fixed header is `const_headers`",
                operation.id
            );
        } else {
            assert!(
                from.is_none(),
                "read `{}` declares a `From` header; PagerDuty requires it on writes only, and a \
                 read asking for a personal email address asks for customer data it cannot use",
                operation.id
            );
            assert!(
                operation.params.header.is_empty(),
                "read `{}` declares caller-supplied headers",
                operation.id
            );
        }
    }

    // No `From` value is authored. The header carries an email address, which is customer data, and
    // this repository authors none — the parameter is declared, never exemplified.
    let source = std::fs::read_to_string(providers_dir().join(format!("{PROVIDER}.toml")))
        .expect("providers/pagerduty.toml is readable");
    assert!(
        !source.contains('@'),
        "providers/pagerduty.toml contains `@`; no email address — real or example — belongs in it"
    );
}

/// **No operation declares a pagination quirk, and the absence is deliberate.**
///
/// PagerDuty pages with `limit`/`offset`. [`Pagination`](connector_spec::Pagination) offers `Page`
/// and `Cursor` and nothing else, and `limit`/`offset` is neither: `Page` describes a page *number*
/// the next request increments by one, while `offset` is a row count the next request advances by
/// `limit`. A `Page { page_param = "offset" }` would build and read correctly to a reviewer while
/// recording something false about the vendor. `Cursor` needs a next-cursor pointer PagerDuty's
/// responses do not carry — they carry `more`, a flag.
///
/// **This is a claim about a declaration, not about emitted code.** No pagination loop is emitted
/// today: `ir.rs:352` says compiling the enum into Flux control flow is C-12's work, and
/// `connector-flux` consults only `quirks.error_envelope`. The pin exists precisely because that
/// day is coming — a false declaration sitting in the file until C-12 lands becomes a wrong loop the
/// moment it does, and it would be bounded by `max_pages` rather than infinite, which makes it
/// harder to notice rather than easier.
///
/// So the parameters ship and the quirk does not — `providers/launchdarkly.toml:123` made the same
/// call for the same vendor shape. This test is the pin: it fails the moment somebody "completes"
/// the connector with a quirk the IR cannot express honestly.
#[test]
fn no_pagerduty_operation_declares_a_pagination_quirk() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.quirks.pagination.is_none(),
            "operation `{}` declares a pagination quirk. PagerDuty pages with `limit`/`offset`, \
             which is neither `Pagination::Page` (a page *number* incremented by one, where \
             `offset` is a row count advanced by `limit`) nor `Pagination::Cursor` (which needs a \
             next-cursor pointer PagerDuty does not send). Either would record something false \
             about the vendor, and C-12 will turn that into a wrong loop — see this file's module \
             docs",
            operation.id
        );
    }

    // The half that must be present: the list operations really do offer `limit` and `offset`, so
    // the absence above is a statement about the IR rather than about PagerDuty.
    for id in [
        "pagerduty-incident-list",
        "pagerduty-service-list",
        "pagerduty-oncall-list",
    ] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("pagerduty declares `{id}`"));
        for expected in ["limit", "offset"] {
            assert!(
                operation
                    .params
                    .query
                    .iter()
                    .any(|param| param.name == expected),
                "list operation `{id}` declares no `{expected}` query parameter; a caller pages by \
                 incrementing `offset` itself"
            );
        }
    }
}

/// **Acknowledging is not resolving, and the two are separate operations carrying separate risk.**
///
/// PagerDuty exposes one endpoint for both — `PUT /incidents/{id}` with `incident.status` — and this
/// connector deliberately splits it. A single operation taking a `status` enum would need one `risk`
/// covering both, and the only honest choice would be the higher of the two: acknowledging says "a
/// human is looking at this" and stops the escalation clock, while resolving closes the incident and
/// takes it off every on-call rotation's board. `medium` and `high` are the two answers, and one
/// operation cannot give both.
///
/// Both are `PUT`s whose body is a pinned constant, so neither takes a caller-supplied status: the
/// operation *is* the status.
#[test]
fn acknowledge_and_resolve_are_separate_operations_with_separate_risk() {
    let connector = load();

    for (id, status, risk) in [
        (
            "pagerduty-incident-acknowledge",
            "acknowledged",
            Risk::Medium,
        ),
        ("pagerduty-incident-resolve", "resolved", Risk::High),
    ] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("pagerduty declares `{id}`"));

        assert_eq!(operation.method, HttpMethod::Put);
        assert_eq!(operation.path, "/incidents/{id}");
        assert_eq!(
            operation.risk, risk,
            "`{id}` must carry its own risk; that separation is the whole reason the vendor's one \
             endpoint is two operations here"
        );
        assert_eq!(
            operation.idempotency,
            Idempotency::Idempotent,
            "`{id}` is a PUT setting a fixed status; repeating it lands in the same state"
        );

        let status_field = operation
            .params
            .body
            .iter()
            .find(|param| param.wire.as_deref() == Some("incident.status"))
            .unwrap_or_else(|| panic!("`{id}` declares no `incident.status` body field"));
        assert_eq!(
            status_field.schema.get("const"),
            Some(&serde_json::json!(status)),
            "`{id}`'s status must be pinned with a JSON Schema `const` — a constant body field is \
             sent and kept out of the op signature, so no model is asked to retype the operation's \
             own name"
        );
    }

    // The two must not have collapsed back into one operation with a caller-supplied status.
    for operation in &connector.operations {
        for param in operation.params.iter() {
            let pinned = param.schema.get("const").is_some();
            assert!(
                !(param.name == "status" && !pinned),
                "operation `{}` takes a caller-supplied `status`; acknowledge and resolve are \
                 separate operations precisely so that one `risk` does not have to cover both",
                operation.id
            );
        }
    }
}

/// The configuration surface asks for exactly what the connector needs and nothing else: the API
/// token, and no second field. The `From` header is **not** here — see this file's module docs.
#[test]
fn the_config_surface_asks_only_for_the_api_token() {
    let connector = load();

    assert_eq!(
        connector.config.len(),
        1,
        "one field: the REST API key. PagerDuty has no per-tenant host to configure, and `From` \
         has no binding to reach"
    );

    let token = connector
        .config
        .iter()
        .find(|field| field.name == "api_token")
        .expect("pagerduty declares an `api_token` config field");
    assert_eq!(token.binds, format!("credential.{CREDENTIAL}"));
    assert!(token.secret, "the API key must be gated as a secret");
    assert!(
        token.example.is_none(),
        "a secret field must carry no realistic-looking example"
    );

    for field in &connector.config {
        assert!(
            !field.label.is_empty() && !field.help.is_empty(),
            "config field `{}` must be renderable: `label` and `help` are mandatory",
            field.name
        );
    }
}

/// **No credential reaches a generated module** — not a value, not a variable name, and not the
/// scheme text that precedes it. The prefix is public words, but it lives in the manifest's auth
/// declaration and has no business in an emitted `op`.
#[test]
fn no_pagerduty_module_carries_a_credential_or_its_variable_name() {
    let connector = load();

    for operation in &connector.operations {
        let text = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        for forbidden in [TOKEN_ENV, CREDENTIAL, "$secret", AUTH_HEADER, AUTH_PREFIX] {
            assert!(
                !text.contains(forbidden),
                "`{}` names `{forbidden}` in generated Flux; a generated module carries no \
                 credential and no credential reference (C-10, AGENTS.md):\n{text}",
                operation.id
            );
        }
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical,
/// and **loads** as exactly one exposed composite op.
#[test]
fn every_pagerduty_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
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
