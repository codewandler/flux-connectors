//! The Stripe connector, and the two properties that make it different from every other shipped
//! provider: **it moves money, and it cannot send a request body.**
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims that are specific to a payments API, because they are the
//! reason C-106 exists and the reason a later reader must not "complete" the connector:
//!
//! - **Stripe takes `application/x-www-form-urlencoded` request bodies and nothing else.** The
//!   emitter sends exactly one media type, `application/json`
//!   (`connector-flux`'s `JSON_MEDIA_TYPE`), so any Stripe operation declaring a body field would
//!   ship a request Stripe answers `400 Missing required param`. The connector is therefore curated
//!   down to endpoints that address everything they need in the *path*, and
//!   [`no_stripe_operation_sends_a_request_body`] is what keeps it there — asserted over the IR *and*
//!   over the emitted text, because a body reintroduced through `body_schema` would leave the field
//!   list empty and still send JSON.
//! - **Every write declares a required `Idempotency-Key` header**, which is what makes
//!   `idempotency = "conditional"` a fact about the connector rather than a hope about its callers.
//!   Stripe's key is optional to Stripe; it is mandatory here, because a retried capture without one
//!   captures twice. [`every_stripe_write_requires_a_caller_supplied_idempotency_key`] asserts the
//!   IR and the emitted header together.
//! - **Risk grades money movement, not the HTTP verb.** A refund is `destructive` — the money has
//!   left and Stripe offers no un-refund — while a capture and a cancel are `high`. Reads are `low`.
//!   [`stripe_grades_its_operations_by_what_they_do_to_money`] pins the whole table, because these
//!   are the values flux's approval gate reads.
//! - **There is no webhook binding, deliberately.** `Stripe-Signature` is a comma-separated
//!   key/value list (`t=…,v1=…`) and [`connector_spec::HmacSpec`] addresses a whole header with one
//!   literal prefix, so the scheme is not expressible — C-60's conformance matrix already declares
//!   Stripe `cannot verify` and C-141 owns the fix. A binding stating `verification = "none"` would
//!   be a security claim that is wrong, so
//!   [`stripe_declares_events_but_no_channel_binding_until_c141`] asserts the absence and names the
//!   reason.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider id, and therefore the file name, the module name and every op id's prefix.
const PROVIDER: &str = "stripe";

/// One tenant-independent host for every account and both key modes, so `base_url` carries no
/// `{template}` for an operator to bind and the connector's whole egress surface is this single name.
const BASE_URL: &str = "https://api.stripe.com";

/// Every selected endpoint lives under Stripe's one version prefix. Asserted so that a later
/// addition cannot quietly address an unversioned path.
const API_PREFIX: &str = "/v1/";

/// The secret key credential, and the environment variable it resolves from. Both are public
/// contract — an operator sets the variable, a manifest names the credential — so they are pinned
/// here rather than left to whatever the file happens to say.
const SECRET_KEY: &str = "stripe.secret_key";
/// See [`SECRET_KEY`]. A variable *name*; no credential value appears in this repository.
const SECRET_KEY_ENV: &str = "STRIPE_SECRET_KEY";

/// The webhook signing secret. Declared and referenced by nothing, exactly as Slack's is: an
/// operator who terminates Stripe webhooks needs this credential, and the credential list is the one
/// place a manifest names everything a connector requires. See
/// [`stripe_declares_events_but_no_channel_binding_until_c141`] for why nothing points at it yet.
const SIGNING_SECRET: &str = "stripe.webhook_signing_secret";
/// See [`SIGNING_SECRET`].
const SIGNING_SECRET_ENV: &str = "STRIPE_WEBHOOK_SIGNING_SECRET";

/// The header Stripe reads a replay guard from, and the caller-facing name it is declared under.
const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";
/// See [`IDEMPOTENCY_HEADER`]. The Flux symbol a write's declaration takes.
const IDEMPOTENCY_PARAM: &str = "idempotency_key";

/// The curated operation set, in published order, each with the risk
/// and idempotency C-106 requires it to carry.
///
/// One table rather than three, because the point of this connector is that the three columns agree:
/// the operations that move money are the ones graded above `low`, and they are exactly the ones
/// that carry a replay guard.
const OPERATIONS: &[(&str, Risk, Idempotency)] = &[
    ("stripe-balance-get", Risk::Low, Idempotency::Idempotent),
    ("stripe-customer-get", Risk::Low, Idempotency::Idempotent),
    ("stripe-charge-get", Risk::Low, Idempotency::Idempotent),
    (
        "stripe-payment-intent-get",
        Risk::Low,
        Idempotency::Idempotent,
    ),
    ("stripe-refund-get", Risk::Low, Idempotency::Idempotent),
    (
        "stripe-payment-intent-capture",
        Risk::High,
        Idempotency::Conditional,
    ),
    (
        "stripe-payment-intent-cancel",
        Risk::High,
        Idempotency::Conditional,
    ),
    (
        "stripe-charge-refund-create",
        Risk::Destructive,
        Idempotency::Conditional,
    ),
    (
        "stripe-country-spec-list",
        Risk::Low,
        Idempotency::Idempotent,
    ),
    ("stripe-event-list", Risk::Low, Idempotency::Idempotent),
    (
        "stripe-exchange-rate-list",
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "stripe-billing-meter-list",
        Risk::Low,
        Idempotency::Idempotent,
    ),
];

/// The only operations allowed to assemble a query string before C-30 lands. Closed by public id:
/// a future operation ending in `-list` earns no permission merely from its name.
const INTEGER_LIMIT_LISTS: [&str; 4] = [
    "stripe-country-spec-list",
    "stripe-event-list",
    "stripe-exchange-rate-list",
    "stripe-billing-meter-list",
];

/// The events C-106 declares. Stripe publishes some 250 event types; these are the four that
/// correspond to the operation surface above.
const EVENTS: &[&str] = &[
    "payment_intent.succeeded",
    "payment_intent.payment_failed",
    "charge.refunded",
    "charge.dispute.created",
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
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-106 ships the Stripe connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// Whether the operation changes state on Stripe's side. The emitter derives write-ness from the verb
/// the same way (`check_write_metadata`), and this connector has no method that disagrees with it.
fn mutates(method: HttpMethod) -> bool {
    !matches!(method, HttpMethod::Get | HttpMethod::Head)
}

/// The connector exists, loads, and is the one C-106 specifies: a bearer secret key over
/// `api.stripe.com`, with the curated operation set and a read-shaped `verify`.
#[test]
fn the_stripe_connector_loads_and_authenticates_with_a_bearer_secret_key() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Stripe");
    assert_eq!(
        connector.base_url, BASE_URL,
        "one host serves every account and both key modes; it is never widened"
    );

    let secret = connector
        .auth_method(SECRET_KEY)
        .unwrap_or_else(|| panic!("stripe declares `{SECRET_KEY}`"));
    assert_eq!(
        secret.scheme,
        AuthScheme::Bearer,
        "Stripe takes `Authorization: Bearer <secret key>`"
    );
    assert_eq!(secret.env, [SECRET_KEY_ENV]);
    assert!(
        secret.user_env.is_empty() && secret.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a bearer has no user half"
    );

    let signing = connector
        .auth_method(SIGNING_SECRET)
        .unwrap_or_else(|| panic!("stripe declares `{SIGNING_SECRET}`"));
    assert_eq!(
        signing.scheme,
        AuthScheme::Signing,
        "a webhook secret never travels outbound; `signing` is the variant that says so"
    );
    assert_eq!(signing.env, [SIGNING_SECRET_ENV]);

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    let expected: Vec<&str> = OPERATIONS.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(
        declared, expected,
        "the operation set is curated and ordered; adding one is a decision, not an omission"
    );

    for operation in &connector.operations {
        assert!(
            operation.path.starts_with(API_PREFIX),
            "operation `{}` addresses `{}`, outside Stripe's one version prefix `{API_PREFIX}`",
            operation.id,
            operation.path
        );
        // Every operation resolves to the secret key alone. The signing secret is inbound-only and
        // the loader refuses an operation that authenticates with it; this asserts the positive half.
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; stripe is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(SECRET_KEY) && effective[0].len() == 1,
            "operation `{}` names a credential other than the secret key",
            operation.id
        );
    }

    // The "Test connection" button. A read with no parameters, so pressing it cannot do anything but
    // prove the key resolves — and it is the read that also reports which mode the key is in.
    assert_eq!(
        connector.verify.as_deref(),
        Some("stripe-balance-get"),
        "`verify` names the parameterless balance read"
    );
}

/// **The headline constraint: Stripe reads form-encoded request bodies, and this pipeline sends
/// JSON.**
///
/// `connector-flux` binds exactly one media type — `content_type = "application/json"` — whenever an
/// operation declares any body, and flux registers no form encoder. So a Stripe operation with a body
/// field would send Stripe a document it does not parse, and Stripe would answer
/// `400 Missing required param: <field>`: a loud failure, but a connector that cannot write.
///
/// The whole operation set is therefore selected around it — every write addresses its subject in the
/// path and takes its remaining parameters' defaults. Stated over the IR *and* over the emitted text,
/// because they fail differently: a `body_schema` leaves `params.body` empty and still emits a
/// payload, and an emitter change could start sending a body the IR never declared.
#[test]
fn no_stripe_operation_sends_a_request_body() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.params.body.is_empty(),
            "operation `{}` declares body fields {:?}. Stripe parses only \
             `application/x-www-form-urlencoded` and this emitter sends only `application/json`, so \
             the fields would not arrive and Stripe would answer `400 Missing required param`. If a \
             request-encoding axis has landed, change this test deliberately",
            operation.id,
            operation
                .params
                .body
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>()
        );
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form `body_schema`, which emits the same JSON body by \
             another route",
            operation.id
        );

        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            !emitted.contains("payload = "),
            "`{}` binds a request payload:\n{emitted}",
            operation.id
        );
        assert!(
            !emitted.contains("content_type"),
            "`{}` sends a content type, which the emitter binds only for a JSON body:\n{emitted}",
            operation.id
        );
    }
}

/// **Every write carries a caller-supplied `Idempotency-Key`, and that is what `conditional` means.**
///
/// [`connector_spec::Idempotency::Conditional`] is documented as "idempotent only under a condition
/// the caller supplies (e.g. an idempotency key)". Declaring it while leaving the key optional would
/// make the claim unfalsifiable — the connector would be conditional on a condition no caller had to
/// meet. So the header is `required`, which is *stricter than Stripe*: Stripe treats the key as
/// optional and this connector does not, because the failure it prevents is a retried capture
/// charging a customer twice.
///
/// The emitted half matters independently: a header param that stopped reaching `http.request` would
/// leave the IR intact and send Stripe a request with no replay guard at all.
#[test]
fn every_stripe_write_requires_a_caller_supplied_idempotency_key() {
    let connector = load();

    let mut writes = 0;
    for operation in &connector.operations {
        if !mutates(operation.method) {
            assert!(
                operation.params.header.is_empty(),
                "read `{}` declares headers {:?}; a read needs no replay guard",
                operation.id,
                operation
                    .params
                    .header
                    .iter()
                    .map(|param| param.name.as_str())
                    .collect::<Vec<_>>()
            );
            continue;
        }
        writes += 1;

        assert_eq!(
            operation.idempotency,
            Idempotency::Conditional,
            "write `{}` is not `conditional`, so nothing says the replay guard below is what makes \
             a retry sound",
            operation.id
        );

        let key = operation
            .params
            .header
            .iter()
            .find(|param| param.name == IDEMPOTENCY_PARAM)
            .unwrap_or_else(|| {
                panic!(
                    "write `{}` declares no `{IDEMPOTENCY_PARAM}` header, so a retry of it repeats \
                     its effect on a customer's money",
                    operation.id
                )
            });
        assert_eq!(
            key.wire.as_deref(),
            Some(IDEMPOTENCY_HEADER),
            "write `{}`: the wire name is Stripe's own header spelling",
            operation.id
        );
        assert!(
            key.required,
            "write `{}`: the idempotency key is optional, so `conditional` is conditional on \
             nothing a caller has to do",
            operation.id
        );
        assert_eq!(
            operation.params.header.len(),
            1,
            "write `{}` declares headers beyond the replay guard; the Authorization header is the \
             host's business at the `$auth` seam and must never travel through the parameter surface",
            operation.id
        );

        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            emitted.contains(&format!(
                r#"headers: {{ "{IDEMPOTENCY_HEADER}": {IDEMPOTENCY_PARAM} }}"#
            )),
            "`{}` does not send the `{IDEMPOTENCY_HEADER}` header as its only header:\n{emitted}",
            operation.id
        );
    }

    assert!(
        writes >= 3,
        "only {writes} stripe operations write, so the claim above is nearly vacuous; C-106 ships \
         three"
    );
}

/// **Risk grades what an operation does to money, not which verb it uses.**
///
/// flux's approval gate reads these values, so the table is the connector's safety contract and not
/// paperwork. Two entries are the substance:
///
/// - `stripe-charge-refund-create` is `destructive` — "deletes or otherwise irreversible". Money
///   leaves the account, the customer's bank is told, and Stripe publishes no un-refund. It is the
///   only shipped operation outside a delete that earns the tier.
/// - `stripe-payment-intent-capture` is `high` and not `medium`. A capture takes an authorization
///   and turns it into a real charge on a real card; `medium` — "writes with limited blast radius" —
///   would wave it past a human.
#[test]
fn stripe_grades_its_operations_by_what_they_do_to_money() {
    let connector = load();

    for (id, risk, idempotency) in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("stripe declares `{id}`"));
        assert_eq!(
            operation.risk, *risk,
            "`{id}` is graded {:?}; C-106 requires {risk:?}",
            operation.risk
        );
        assert_eq!(
            operation.idempotency, *idempotency,
            "`{id}` declares {:?} idempotency; C-106 requires {idempotency:?}",
            operation.idempotency
        );
        assert_eq!(
            mutates(operation.method),
            *risk != Risk::Low,
            "`{id}`: the risk tier and the HTTP verb disagree about whether this is a write"
        );
    }
}

/// **No injectable query parameter** — the strong form, on the IR and on every emitted `url`
/// binding.
///
/// Nothing in this pipeline percent-encodes a query value (C-30), and `zendesk-ticket-search` is the
/// standing demonstration AGENTS.md lists under *Intentional gaps*. Stripe's collection surface is
/// exactly that shape — `GET /v1/charges?customer=…&created[gte]=…` uses bracketed nested keys that
/// need encoding to survive at all. C-470's four lists expose only integer `limit`, whose decimal
/// rendering cannot add a query pair; every cursor, string, boolean, array and object filter stays
/// omitted.
///
/// Every `url = ` line is checked, not just the first: the emitter re-binds `$url` once per *optional*
/// query parameter inside a `when` guard, so inspecting only the first binding would pass while an
/// operation quietly appended filters.
#[test]
fn stripe_queries_are_absent_or_the_one_safe_integer_limit() {
    let connector = load();

    for operation in &connector.operations {
        let declared: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        let is_list = INTEGER_LIMIT_LISTS.contains(&operation.id.as_str());
        if is_list {
            assert_eq!(
                declared,
                ["limit"],
                "{} widened its query surface",
                operation.id
            );
            assert_eq!(
                operation.params.query[0].schema["type"],
                serde_json::json!("integer"),
                "{} can interpolate limit safely only while it is numeric",
                operation.id
            );
        } else {
            assert!(
                declared.is_empty(),
                "operation `{}` declares query parameters {declared:?}. Nothing percent-encodes a \
                 query value (C-30); only the four reviewed integer-limit lists may carry one",
                operation.id
            );
        }

        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        if is_list {
            assert_eq!(
                url_lines.len(),
                1,
                "{} put query data in its URL",
                operation.id
            );
            assert!(emitted.contains("query: { limit }"));
            assert!(!emitted.contains("sep = "));
        } else {
            assert_eq!(
                url_lines.len(),
                1,
                "{} unexpectedly appends a query",
                operation.id
            );
            assert!(!url_lines[0].contains('?') && !emitted.contains("sep = "));
        }
    }
}

/// **The events are declared; the webhook binding is not, and that is the finding.**
///
/// `Stripe-Signature` is a comma-separated key/value list — `t=1614556800,v1=5257a869e7…` — carrying
/// the timestamp and one digest per scheme version. [`connector_spec::HmacSpec`] has a single literal
/// `prefix` and a [`connector_spec::Selector`] addressing a **whole header**; neither can take a
/// component out of that list, and Stripe sends more than one `v1` during a secret rotation, of which
/// a verifier must accept any. C-60's conformance matrix
/// (`crates/connector-spec/tests/verification_conformance.rs`) already declares Stripe `cannot
/// verify` rather than pretending, and C-141 owns the extraction axis that closes it.
///
/// The two available shortcuts are both refused. A binding with `verification = "none"` would present
/// an unverified public endpoint as trusted, on a payments webhook, which is the worst claim in this
/// repository to get wrong. A binding with an `hmac` block naming `Stripe-Signature` whole would
/// verify a digest against a string that is not the digest, and fail closed on every genuine delivery
/// while *reading* as though verification were implemented.
///
/// So the events ship — they are what a trigger matches on and they are correct today — and the
/// binding waits. This test is the tripwire: when C-141 lands and a `[[channels]]` block appears, it
/// fails, and whoever adds the binding has to come back and say why it is now expressible.
#[test]
fn stripe_declares_events_but_no_channel_binding_until_c141() {
    let connector = load();

    let declared: Vec<&str> = connector
        .events
        .iter()
        .map(|event| event.name.as_str())
        .collect();
    assert_eq!(
        declared, EVENTS,
        "the event set is curated from some 250 Stripe publishes and mirrors the operation surface"
    );

    assert!(
        connector.channels.is_empty(),
        "stripe declares a channel binding. `Stripe-Signature` packs `t=` and one or more `v1=` \
         components into one header, which `HmacSpec`'s single literal `prefix` and whole-header \
         `Selector` cannot address — C-60 records Stripe as `cannot verify` and C-141 owns the fix. \
         A binding stating `verification = \"none\"` would present an unverified payments endpoint \
         as trusted. If C-141 has landed, change this test deliberately"
    );

    // The credential the binding will need is already declared, so an operator provisioning Stripe
    // is told about it once rather than discovering it when the binding lands.
    assert!(
        connector.auth_method(SIGNING_SECRET).is_some(),
        "the webhook signing secret is declared even while unreferenced, as Slack's is"
    );
}

/// **No credential value, and no realistic-looking placeholder on a secret field.**
///
/// A placeholder shaped like a real Stripe key is the failure this repository has already had: a
/// `shpat_`-prefixed example tripped GitHub's push protection and blocked a release. Stripe keys are
/// the most heavily scanned secret there is, so a secret field here carries no `example` at all and
/// the shape lives in `help`, where it cannot be mistaken for a value.
#[test]
fn no_stripe_config_field_offers_a_credential_shaped_example() {
    let connector = load();

    assert!(
        !connector.config.is_empty(),
        "stripe declares configuration; a connector nobody can configure is not shippable"
    );

    for field in &connector.config {
        if field.secret {
            assert!(
                field.example.is_none(),
                "config field `{}` is secret and carries the example {:?}. A placeholder on a \
                 secret field is copied by users and scanned by push protection; put the shape in \
                 `help`",
                field.name,
                field.example
            );
        }
        assert!(
            !field.label.is_empty() && !field.help.is_empty(),
            "config field `{}` is not renderable without a label and help",
            field.name
        );
        for text in [&field.help, &field.label] {
            for prefix in ["sk_live_", "sk_test_", "rk_live_", "whsec_"] {
                let Some(rest) = text.split(prefix).nth(1) else {
                    continue;
                };
                let run = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .count();
                assert!(
                    run <= 4,
                    "config field `{}` carries `{prefix}` followed by {run} characters, which reads \
                     as a real key. Name the prefix and stop",
                    field.name
                );
            }
        }
    }
}

/// **The C-11 gate, per operation.** Each one emits, parses with no diagnostics, and is already a
/// fixed point of flux's own formatter.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set; it is restated here so the Stripe
/// connector's own file fails on its own when its modules stop being analyzable.
#[test]
fn every_stripe_operation_emits_an_analyzable_module() {
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
            "`{}` is not a fixed point of flux's own formatter, so the generated module would be \
             rewritten the first time anyone opened it",
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
