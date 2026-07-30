//! An operation declares **how its request body is encoded**, and the emitter obeys it (C-144).
//!
//! Every assertion here runs the whole authoring path — provider TOML → IR → emitted Flux — rather
//! than assembling an IR in memory. That is deliberate: the gap C-144 measured was that there was no
//! *key* to write, so a test that only constructed a Rust value would pass on an axis no provider
//! file could reach.
//!
//! # Why a form body is `fmt`, and what that costs
//!
//! flux has no form encoder, and this was checked rather than assumed. Under flux-lang 0.39 the only
//! node that produces text from a record is `parse(x, as: "json")`, whose `as_type` the analyzer
//! restricts to `f64`/`i64`/`bool`/`json`/`string`
//! (`../flux/crates/flux-lang/src/analyze.rs:1809-1815`) — so `as: "form"` is not a spelling that
//! analyzes, it is a spelling that fails. There is no `encode`/`stringify`/`serialize` node, no
//! `expr` function that escapes anything (`../flux/crates/flux-lang/src/expr.rs:804-828`), no
//! registered op in flux's core catalogue that percent-encodes, and `http.request` does not serialize
//! a record for you — it reads `body` with `Value::as_str` and forwards the bytes verbatim
//! (`../flux/crates/flux-web/src/http.rs:183-186`, `egress.rs:83-85`).
//!
//! So the one construction available is the one the query string already uses: `fmt`, with each value
//! interpolated. **That leaves form values unencoded, exactly as query values are unencoded** — the
//! gap AGENTS.md records for `zendesk-ticket-search`, now reaching a second request position. It is
//! recorded, not papered over: half-encoding in emitted Flux would look correct and be wrong, and
//! hand-rolling percent-encoding out of `replace` chains is the connector-specific DSL this
//! repository refuses. The real fix is a flux-side encoder, and it belongs on flux's board next to
//! the structured-`query` story in `docs/designs/query-encoding-flux-stories.md`.

use connector_spec::{provider, Connector};

/// A form-encoded vendor: two always-present body fields and one optional one, which is the shape
/// Stripe's writes and every OAuth2 token endpoint have.
const FORM: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A vendor that parses only form-encoded bodies"

[[operations]]
id = "acme-charge-create"
method = "POST"
path = "/v1/charges"
description = "Charge a customer"
risk = "medium"
idempotency = "non_idempotent"

[operations.params]
body_encoding = "form"

[[operations.params.body]]
name = "amount"
description = "Amount in the currency's smallest unit"
required = true
schema = { type = "integer", minimum = 1 }

[[operations.params.body]]
name = "currency"
description = "Three-letter ISO currency code"
required = true
schema = { type = "string" }

[[operations.params.body]]
name = "note"
description = "An arbitrary note stored on the charge"
required = false
schema = { type = "string" }
"#;

/// The same operation with nothing declared — the default, which must stay JSON.
const DEFAULTED: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A vendor that parses JSON"

[[operations]]
id = "acme-charge-create"
method = "POST"
path = "/v1/charges"
description = "Charge a customer"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "amount"
description = "Amount in the currency's smallest unit"
required = true
schema = { type = "integer", minimum = 1 }
"#;

fn load(source: &str) -> Connector {
    provider::load("providers/acme.toml", source)
        .unwrap_or_else(|error| panic!("the fixture must load: {error}"))
        .connector
}

fn emit(source: &str) -> String {
    let connector = load(source);
    connector_flux::emit_operation(&connector, &connector.operations[0])
        .unwrap_or_else(|error| panic!("the fixture must emit: {error}"))
}

/// **The story's failing-first test.** Before C-144 the emitter bound one media type
/// unconditionally, so this could not pass by any route: the IR had no key to declare an encoding
/// with, and `application/json` was a `const`.
#[test]
fn a_form_encoded_operation_emits_a_form_body_not_json() {
    let emitted = emit(FORM);

    assert!(
        emitted.contains(r#"content_type = "application/x-www-form-urlencoded""#),
        "the declared encoding must reach the media type:\n{emitted}"
    );
    assert!(
        !emitted.contains("application/json"),
        "no JSON media type may survive a form declaration:\n{emitted}"
    );
    // The pairs, not a record. A record would reach `http.request` as canonical JSON text under a
    // form `content-type` — a body the vendor answers `400` to, or worse, ignores.
    assert!(
        emitted.contains(r#"payload = fmt("amount={amount}&currency={currency}")"#),
        "the always-present fields must be assembled as form pairs:\n{emitted}"
    );
    assert!(
        !emitted.contains("payload = {"),
        "a form body must not be assembled as a record:\n{emitted}"
    );
    // An unsupplied optional field must not travel as the literal text `note=null`.
    assert!(
        emitted.contains(r#"payload = fmt("{payload}&note={note}")"#),
        "an optional field must be appended under a guard:\n{emitted}"
    );
    // And the header the emitter owns still describes the body it actually assembled.
    assert!(
        emitted.contains(r#"headers: { "content-type": content_type }"#),
        "the media type must travel in the content-type header:\n{emitted}"
    );
    assert!(
        emitted.contains("body: payload"),
        "the assembled text must be the request body:\n{emitted}"
    );
}

/// `json` is the default, and the default is what every shipped provider already relies on.
///
/// `shipped_modules.rs` holds the stronger form of this claim — every committed rendering is
/// byte-identical — but this states the mechanism directly: an operation that declares nothing gets
/// the JSON record and the JSON media type it got before the axis existed.
#[test]
fn an_operation_that_declares_no_encoding_still_emits_json() {
    let emitted = emit(DEFAULTED);

    assert!(
        emitted.contains(r#"content_type = "application/json""#),
        "JSON must remain the default media type:\n{emitted}"
    );
    assert!(
        emitted.contains("payload = { amount }"),
        "a JSON body must remain a record:\n{emitted}"
    );
}

/// **Nesting is refused, not flattened.** `application/x-www-form-urlencoded` has no agreed nesting
/// convention — Stripe writes `metadata[key]`, PHP writes `a[b]`, Rails writes `a[b][]`, and OAuth2
/// nests nothing at all — so picking one here would send a vendor a key it does not recognise and be
/// answered `200`.
#[test]
fn a_nested_field_cannot_be_form_encoded() {
    let nested = FORM.replace(
        r#"name = "note""#,
        "name = \"note\"\nwire = \"charge.note\"",
    );
    let connector = load(&nested);
    let error = connector_flux::emit_operation(&connector, &connector.operations[0])
        .expect_err("a dotted wire path under a form encoding must be refused");
    assert!(
        matches!(error, connector_flux::Error::UnencodableFormField { .. }),
        "expected a form-field refusal, got: {error}"
    );
}

/// The key is flat but the **value** nests, which is the same problem one level down: interpolated,
/// an object would reach the vendor as JSON text inside a form pair, and no form parser reassembles
/// that.
#[test]
fn a_field_whose_value_nests_cannot_be_form_encoded() {
    for ty in ["object", "array"] {
        let nested = FORM.replace(
            r#"name = "note"
description = "An arbitrary note stored on the charge"
required = false
schema = { type = "string" }"#,
            &format!(
                r#"name = "metadata"
description = "Arbitrary key/value pairs stored on the charge"
required = false
schema = {{ type = "{ty}" }}"#
            ),
        );
        let connector = load(&nested);
        let error =
            connector_flux::emit_operation(&connector, &connector.operations[0]).unwrap_err();
        assert!(
            matches!(error, connector_flux::Error::UnencodableFormField { .. }),
            "a declared `{ty}` value must be refused, got: {error}"
        );
    }
}

/// A free-form body names no fields, so there is no template to build. Under JSON the caller's whole
/// record is canonicalised by `parse($body, as: "json")`; under `form` there is no equivalent, and
/// inventing one would be this emitter claiming an encoding it cannot produce.
#[test]
fn a_free_form_body_cannot_be_form_encoded() {
    let free_form = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A vendor that parses only form-encoded bodies"

[[operations]]
id = "acme-charge-create"
method = "POST"
path = "/v1/charges"
description = "Charge a customer"
risk = "medium"
idempotency = "non_idempotent"

[operations.params]
body_encoding = "form"
body_schema = { type = "object" }
"#;
    let connector = load(free_form);
    let error = connector_flux::emit_operation(&connector, &connector.operations[0])
        .expect_err("a free-form body under a form encoding must be refused");
    assert!(
        matches!(error, connector_flux::Error::UnencodableFormBody { .. }),
        "expected a free-form refusal, got: {error}"
    );
}

/// An encoding declared on an operation that sends no body is a declaration that changes nothing —
/// the same silent no-op `const` on a header parameter used to be, and refused for the same reason.
#[test]
fn an_encoding_declared_without_a_body_is_refused() {
    let bodiless = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A vendor that parses only form-encoded bodies"

[[operations]]
id = "acme-charge-list"
method = "GET"
path = "/v1/charges"
description = "List charges"
risk = "low"
idempotency = "idempotent"

[operations.params]
body_encoding = "form"
"#;
    let connector = load(bodiless);
    let error = connector_flux::emit_operation(&connector, &connector.operations[0])
        .expect_err("an encoding with no body to encode must be refused");
    assert!(
        matches!(error, connector_flux::Error::BodyEncodingWithoutBody { .. }),
        "expected a bodiless-encoding refusal, got: {error}"
    );
}

/// **The set is closed.** An open string would be a media type nobody validates, and a typo would
/// ship a body the vendor silently ignores — so an unknown encoding is a load error, not a fallback
/// to JSON.
#[test]
fn an_unknown_encoding_does_not_load() {
    let error = provider::load(
        "providers/acme.toml",
        &FORM.replace(
            r#"body_encoding = "form""#,
            r#"body_encoding = "multipart""#,
        ),
    )
    .expect_err("an encoding outside the closed set must not load");
    assert!(
        error.to_string().contains("body_encoding"),
        "the refusal must name the key that is wrong: {error}"
    );
}

/// A form body whose **every** field is optional carries its own separator, exactly as the query
/// string does — because the first *surviving* pair must not be preceded by an `&`, and which pair
/// that is cannot be known at emit time.
#[test]
fn a_form_body_of_only_optional_fields_carries_its_own_separator() {
    let emitted = emit(&FORM.replace("required = true", "required = false"));

    assert!(
        emitted.contains(r#"payload = """#) && emitted.contains(r#"form_sep = """#),
        "an all-optional form body opens empty:\n{emitted}"
    );
    assert!(
        emitted.contains(r#"payload = fmt("{payload}{form_sep}amount={amount}")"#),
        "the first pair must go through the carried separator:\n{emitted}"
    );
    // The last pair never hands a separator on, so exactly the two earlier guards set it.
    assert_eq!(
        emitted.matches(r#"form_sep = "&""#).count(),
        2,
        "only a pair that another could follow needs to set the separator:\n{emitted}"
    );

    let parsed = flux_lang::parser::parse_cst(&emitted);
    assert!(parsed.errors.is_empty(), "{:?}\n{emitted}", parsed.errors);
    assert_eq!(
        flux_lang::format_cst::format_module(&parsed).as_deref(),
        Some(emitted.as_str()),
        "the flux formatter would rewrite an all-optional form body"
    );
}

/// The C-11 gate, for the shape only this file produces: a form body still parses, still loads as one
/// exposed composite op, and is still a fixed point of flux's own formatter.
#[test]
fn a_form_body_parses_analyzes_and_is_canonical() {
    let emitted = emit(FORM);

    let parsed = flux_lang::parser::parse_cst(&emitted);
    assert!(
        parsed.errors.is_empty(),
        "a form body must parse: {:?}\n{emitted}",
        parsed.errors
    );
    assert_eq!(
        flux_lang::format_cst::format_module(&parsed).as_deref(),
        Some(emitted.as_str()),
        "the flux formatter would rewrite a form body"
    );

    let module = flux_lang::program::Module::parse_str(&emitted)
        .unwrap_or_else(|error| panic!("a form body must load: {error}"));
    let program = module.program().expect("the module is a program");
    assert_eq!(program.ops.len(), 1);
    assert_eq!(program.ops[0].name, "acme-charge-create");
    assert!(program.ops[0].meta.expose);
}
