//! Klaviyo (C-218) is the epic's probe for a **required, dated API-revision header** — `revision:
//! 2026-07-15` on every request, with no default, no grace period and no unversioned alias.
//!
//! The mechanism is not new. `const_headers` landed with C-55 and `providers/notion.toml`,
//! `providers/anthropic.toml` and `providers/github.toml` each pin a vendor constant through it;
//! Notion's `Notion-Version: 2022-06-28` is already a *dated* one. The story's framing — "nothing
//! shipped has one" — is therefore wrong about the mechanism, and this file does not re-prove it.
//!
//! What is genuinely new here is the **coupling**, and it is what these tests exist to hold:
//!
//! 1. **The revision is a constant, not an operator's choice.** It is declared in `const_headers`
//!    and reaches every emitted request as a literal, and it is spellable *nowhere else* — no
//!    `[[config]]` field offers it, and no operation declares it as a caller-supplied
//!    `params.header`. Both alternatives are pinned as refused so the decision cannot be reversed by
//!    editing one word (`providers/klaviyo.toml`'s header records the reasoning in full).
//! 2. **The pin and the response schemas name one date.** A `revision` is a claim this repository
//!    makes about which API contract the schemas below describe. Bumping the header without
//!    re-reading the schemas asserts a compatibility nobody checked, so the constant is tied to the
//!    provenance sentence in the provider file that says the schemas were read at that revision:
//!    changing either alone fails here.
//! 3. **The key's scopes are stated where an operator reads them.** Klaviyo private API keys are
//!    scoped per key, so a key minted without `profiles:write` fails on the write and nowhere else.
//!    The `help` text names every scope the shipped set needs, and this file checks the list against
//!    the operations rather than against a comment.
//!
//! The structural claims (emitted Flux parses, formats to a fixed point and loads as one op;
//! `verify` is an argument-free read; no query parameter anywhere) deliberately duplicate what
//! `shipped_modules.rs` asserts across the whole catalogue. That file's subject moves when the
//! shipped set does; this one names only klaviyo.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Binding, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider under test.
const PROVIDER: &str = "klaviyo";

/// The vendor's own spelling of the header. Lowercase, and Klaviyo documents it that way.
const REVISION_HEADER: &str = "revision";

/// **The version pin.** Klaviyo versions its whole API by this dated header, and the response
/// schemas in `providers/klaviyo.toml` were read from the developer portal rendered at exactly this
/// revision. The two move together or not at all — see [`the_pin_and_the_schemas_name_one_date`].
const REVISION: &str = "2026-07-15";

/// The one credential, placed on `Authorization` behind Klaviyo's own scheme word.
const CREDENTIAL: &str = "klaviyo.api_key";
/// A variable *name*; no credential value appears in this repository.
const CREDENTIAL_ENV: &str = "KLAVIYO_API_KEY";
/// Klaviyo's scheme word, including its trailing space. Public API syntax, printed in the vendor's
/// own documentation — never a secret.
const AUTH_PREFIX: &str = "Klaviyo-API-Key ";

const BASE_URL: &str = "https://a.klaviyo.com/api";

/// The verification read — argument-free, so a settings page can run it unattended.
const VERIFY: &str = "klaviyo-account-list";

/// The curated set, in the order `providers/klaviyo.toml` declares them.
const OPERATIONS: &[&str] = &[
    "klaviyo-account-list",
    "klaviyo-profile-list",
    "klaviyo-profile-get",
    "klaviyo-profile-create",
    "klaviyo-list-list",
    "klaviyo-event-create",
];

/// Every scope the shipped set needs, and the operation that needs it. A key minted without one of
/// these fails on that operation alone, which is the failure mode `help` has to pre-empt.
const REQUIRED_SCOPES: &[(&str, &str)] = &[
    ("accounts:read", "klaviyo-account-list"),
    ("profiles:read", "klaviyo-profile-list"),
    ("profiles:write", "klaviyo-profile-create"),
    ("lists:read", "klaviyo-list-list"),
    ("events:write", "klaviyo-event-create"),
];

fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join(format!("{PROVIDER}.toml"))
}

fn source() -> String {
    let path = provider_path();
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-218 ships the Klaviyo connector",
            path.display()
        )
    })
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    shipped_provider::load_definition(PROVIDER, &source())
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The declaration line, which is where a caller-supplied argument would show up.
fn signature(emitted: &str) -> &str {
    emitted.lines().next().expect("a declaration line")
}

/// The `headers:` record of the emitted `http.request` call, without its braces.
///
/// flux's formatter puns a record key against a binding of the same name, so a header whose vendor
/// spelling is already a legal Flux symbol is emitted as shorthand — `{ revision }`, exactly as
/// `{ Accept }` is emitted across the shipped catalogue — while `Notion-Version`, which is not,
/// keeps an explicit `"Notion-Version": Notion_Version`. Both spell the same wire key. So the
/// assertions below read the record and ask which keys it binds, rather than pinning one of two
/// renderings that flux, not this repository, chooses between.
fn headers_record(emitted: &str) -> &str {
    let request = emitted
        .lines()
        .find(|line| line.contains("http.request("))
        .expect("every operation emits an http.request call");
    let open = request
        .find("headers: {")
        .map(|at| at + "headers: {".len())
        .unwrap_or_else(|| panic!("no `headers:` record in:\n{request}"));
    let close = request[open..]
        .find('}')
        .map(|at| open + at)
        .unwrap_or_else(|| panic!("unterminated `headers:` record in:\n{request}"));
    request[open..close].trim()
}

/// Whether the emitted `headers:` record sends `name` — under either rendering.
fn sends_header(emitted: &str, name: &str) -> bool {
    let explicit = format!("\"{name}\": {name}");
    headers_record(emitted)
        .split(',')
        .map(str::trim)
        .any(|entry| entry == name || entry == explicit)
}

fn emit(connector: &Connector, id: &str) -> String {
    let operation = connector
        .operation(id)
        .unwrap_or_else(|| panic!("klaviyo declares `{id}`"));
    emit_operation(connector, operation)
        .unwrap_or_else(|error| panic!("`{id}` is not emittable: {error}"))
}

/// The connector exists, loads, and is the one C-218 describes.
#[test]
fn the_klaviyo_connector_loads() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Klaviyo");
    assert_eq!(
        connector.base_url, BASE_URL,
        "one tenant-independent host, never widened"
    );
    assert_eq!(connector.authority.as_deref(), Some("com.klaviyo.api"));

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        declared, OPERATIONS,
        "the curated set, in declaration order. Every exclusion in providers/klaviyo.toml has a \
         recorded reason, so adding one is a deliberate edit here too"
    );
}

/// **Finding 1a: the revision reaches every emitted request as a literal.**
///
/// Klaviyo refuses a request carrying no `revision` header — every request, every endpoint. So this
/// is not a pin that protects against a future default the way github's `Accept` does; it is the
/// connector working at all. Both halves are checked and they fail differently: a missing literal
/// is a refusal on the first call, and a surviving parameter is a required argument a model has to
/// guess, which is the same refusal one call later with a worse tool contract in between.
#[test]
fn the_revision_reaches_every_emitted_operation_as_a_literal() {
    let connector = load();
    let expected_binding = format!(r#"{REVISION_HEADER} = "{REVISION}""#);

    for id in OPERATIONS {
        let emitted = emit(&connector, id);

        assert!(
            emitted.contains(&expected_binding),
            "`{id}` does not bind the revision as a literal. Klaviyo refuses every request that \
             carries no `revision` header, so an operation missing it cannot work at all:\n{emitted}"
        );
        assert!(
            sends_header(&emitted, REVISION_HEADER),
            "`{id}` binds the revision but never sends it — the literal must reach the request \
             under Klaviyo's own spelling. Its `headers:` record is `{}`:\n{emitted}",
            headers_record(&emitted)
        );
        assert!(
            !signature(&emitted).contains(REVISION_HEADER),
            "`{id}` declares the revision as a caller-supplied argument. It is a constant of the \
             connector, not an input:\n{}",
            signature(&emitted)
        );
    }
}

/// **Finding 1b: the revision is a `const_headers` entry and is spellable nowhere else.**
///
/// This is the decision C-218 asks for, pinned rather than described. A `[[config]]` field with a
/// default was the alternative, and it is refused on the merits — an operator has no basis to
/// choose a date, and a date they *could* choose is a date they could choose wrongly, silently
/// invalidating every response schema below. So:
///
/// - it is in `const_headers`, at provider level, distributed by the loader onto every operation;
/// - no operation declares it as a caller-supplied `params.header` (C-55's refused spelling);
/// - no `[[config]]` field mentions it, which is the spelling this story had to rule out.
#[test]
fn the_revision_is_a_constant_and_not_an_operator_choice() {
    let connector = load();

    for operation in &connector.operations {
        assert_eq!(
            operation.params.const_headers.get(REVISION_HEADER),
            Some(&REVISION.to_string()),
            "`{}` does not declare `{REVISION_HEADER}` in `const_headers`. The provider-level table \
             is distributed onto every operation by the loader, so an operation missing it means \
             the table was removed or overridden",
            operation.id
        );

        for header in &operation.params.header {
            let name = header.name.to_ascii_lowercase();
            let wire = header
                .wire
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            assert!(
                name != REVISION_HEADER && wire != REVISION_HEADER,
                "`{}` declares the revision as a caller-supplied header parameter `{}`. \
                 `params.header` means caller-supplied; a vendor constant belongs in \
                 `const_headers`",
                operation.id,
                header.name
            );
        }
    }

    for field in &connector.config {
        assert!(
            !field.name.contains(REVISION_HEADER),
            "config field `{}` offers the revision to an operator. An operator has no basis to \
             choose an API revision, and a date they could choose is a date they could choose \
             wrongly — which would silently invalidate every response schema this connector \
             declares",
            field.name
        );
    }
}

/// **Finding 2, and the reason this connector is worth its slot: the pin and the schemas name one
/// date.**
///
/// A `revision` is not decoration. It is a claim about *which* API contract the response schemas
/// below describe, and it is the only such claim in this catalogue that the vendor itself enforces.
/// A revision bumped without re-reading the schemas is worse than no revision at all: it asserts a
/// compatibility that was never checked, and it does so in a header the vendor will happily accept.
///
/// There is no way to verify a schema against a vendor from inside this repository — that is C-14's
/// standing caveat, and hand-authored vendor shapes drift undetectably. What *can* be held is that
/// the two facts move together: the constant in `const_headers` and the provenance sentence saying
/// the schemas were read at that revision are the same string, so bumping one alone fails here and
/// bumping both means someone edited the sentence claiming they re-read the schemas.
#[test]
fn the_pin_and_the_schemas_name_one_date() {
    let connector = load();
    let source = source();

    let pinned = connector
        .operations
        .first()
        .expect("the curated set is not empty")
        .params
        .const_headers
        .get(REVISION_HEADER)
        .expect("the revision is pinned")
        .clone();
    assert_eq!(pinned, REVISION);

    let provenance =
        format!("Response schemas below were read from the developer portal rendered at revision `{REVISION}`");
    assert!(
        source.contains(&provenance),
        "providers/klaviyo.toml pins `{REVISION_HEADER}: {pinned}` but does not carry the \
         provenance sentence naming that same revision:\n\n    {provenance}\n\nThe two are one \
         claim. Bumping the revision means re-reading the response schemas at the new revision and \
         re-writing this sentence — not editing a date"
    );
}

/// **Finding 3: the key's scopes are named where an operator reads them.**
///
/// Klaviyo private API keys carry per-key scopes, so a key minted read-only authenticates fine and
/// then fails on `klaviyo-profile-create` alone — the worst shape of failure for a settings page,
/// because `verify` passes. The scopes therefore belong in `help`, and the list is checked here
/// against the operations that need them rather than against a comment that can rot.
#[test]
fn the_help_text_names_every_scope_the_shipped_set_needs() {
    let connector = load();

    let field = connector
        .config
        .iter()
        .find(|field| field.name == "api_key")
        .expect("the private API key is the one thing a human must supply");

    for (scope, operation) in REQUIRED_SCOPES {
        assert!(
            field.help.contains(scope),
            "`help` does not name the `{scope}` scope, which `{operation}` needs. A key minted \
             without it authenticates, passes `verify`, and fails on that one call"
        );
        assert!(
            connector.operation(operation).is_some(),
            "the scope list names `{operation}`, which this connector does not declare"
        );
    }
}

/// The credential: one private API key, on `Authorization` behind Klaviyo's own scheme word.
///
/// `Klaviyo-API-Key ` is a prefix, not a `Bearer` — Klaviyo refuses a bearer-prefixed value — and
/// the prefix axis (C-184) is what expresses it without a template. The word is public API syntax
/// from the vendor's documentation; the secret is appended by the host and never appears here.
#[test]
fn one_private_api_key_travels_behind_klaviyos_own_scheme_word() {
    let connector = load();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        [CREDENTIAL],
        "one credential covers the whole curated set"
    );

    let key = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("klaviyo declares `{CREDENTIAL}`"));
    assert_eq!(
        key.scheme,
        AuthScheme::Header {
            name: "Authorization".to_string(),
            prefix: AUTH_PREFIX.to_string(),
        },
        "Klaviyo's scheme word is its own, and a `Bearer `-prefixed value is refused by the vendor"
    );
    assert_eq!(key.env, [CREDENTIAL_ENV]);

    assert_eq!(
        connector.default_auth.len(),
        1,
        "Klaviyo offers one mechanism for a server-side call, so there is one alternative"
    );
    for id in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("klaviyo declares `{id}`"));
        let effective: Vec<Vec<&str>> = connector
            .effective_auth(operation)
            .iter()
            .map(|requirement| requirement.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(
            effective,
            vec![vec![CREDENTIAL]],
            "every operation carries the key; none overrides the default"
        );
    }
}

/// The configuration surface: one secret field, renderable, and **carrying no example**.
///
/// A Klaviyo private key is `pk_` followed by a long hex string, and a placeholder of that shape
/// matches secret scanning — `providers/shopify.toml` records the release this repository actually
/// lost that way. The shape belongs in `help`, where it cannot be copied into a form as a value.
#[test]
fn the_key_is_configurable_secret_and_carries_no_example() {
    let connector = load();

    let fields: Vec<&str> = connector
        .config
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(
        fields,
        ["api_key"],
        "one field: Klaviyo's host is tenant-independent, so nothing else has to be supplied"
    );

    let field = connector.config.first().expect("the one field");
    assert_eq!(
        field.binding(),
        Some(Binding::Credential { name: CREDENTIAL })
    );
    assert!(
        field.secret,
        "a field binding a credential is a secret field — the agreement is a loader rule, not a \
         convention"
    );
    assert!(
        !field.label.is_empty() && !field.help.is_empty(),
        "a field must be renderable: `label` and `help` are what a settings page shows"
    );
    assert_eq!(
        field.example, None,
        "a secret field carries no example — a token-shaped placeholder trips secret scanning and \
         teaches a reader to paste something that looks like a real value"
    );
}

/// `verify` is a read that runs unattended.
///
/// `GET /api/accounts` is Klaviyo's own "which account is this key for" call: it takes no argument,
/// needs only `accounts:read`, and returns the account the key belongs to — which is exactly what a
/// "Test connection" button should establish.
#[test]
fn verify_is_an_argument_free_read() {
    let connector = load();

    assert_eq!(connector.verify.as_deref(), Some(VERIFY));
    let operation = connector
        .operation(VERIFY)
        .expect("verify names a declared operation");

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

/// Every operation declares `risk` and `idempotency`, and a `description` a model can act on.
///
/// The pairing is the checked part: a `GET` that claimed `non_idempotent`, or a write that claimed
/// `low`, would be a metadata claim no reviewer reads twice.
#[test]
fn every_operation_declares_its_effects() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.description.len() > 40,
            "`{}` has no description a model could act on: {:?}",
            operation.id,
            operation.description
        );
        if operation.method == HttpMethod::Get {
            assert_eq!(
                operation.risk,
                Risk::Low,
                "`{}` is a read, so it is `low`",
                operation.id
            );
            assert_eq!(
                operation.idempotency,
                Idempotency::Idempotent,
                "`{}` is a GET; repeating it changes nothing",
                operation.id
            );
        } else {
            assert_ne!(
                operation.risk,
                Risk::Low,
                "`{}` writes to a customer's marketing data; `low` would understate it",
                operation.id
            );
            assert_eq!(
                operation.idempotency,
                Idempotency::NonIdempotent,
                "`{}` earns `idempotent` only from a vendor-documented guarantee, and Klaviyo \
                 documents none for this endpoint",
                operation.id
            );
        }
    }
}

/// No klaviyo operation declares a query parameter.
///
/// Nothing in this pipeline percent-encodes a query value (C-30; `zendesk-ticket-search` is the
/// standing gap under `AGENTS.md`'s Intentional gaps), so a value carrying `&` or `#` corrupts the
/// request. That is what keeps Klaviyo's whole filter/sparse-fieldset/cursor surface out of this
/// connector — every one of those is a query parameter, and `filter` in particular is a small
/// expression language whose values are caller free text.
#[test]
fn no_operation_puts_anything_in_the_query_string() {
    let connector = load();

    for id in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("klaviyo declares `{id}`"));
        assert!(
            operation.params.query.is_empty(),
            "`{id}` declares a query parameter. Every query value this emitter writes is \
             interpolated verbatim"
        );

        let emitted = emit(&connector, id);
        assert!(
            !emitted.contains('?'),
            "`{id}` emits a `?` into its URL:\n{emitted}"
        );
    }
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op — the C-11 gate, held against klaviyo specifically.
#[test]
fn every_klaviyo_operation_emits_an_analyzable_module() {
    let connector = load();

    for id in OPERATIONS {
        let emitted = emit(&connector, id);

        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "`{id}` emits Flux that does not parse: {:?}\n{emitted}",
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str()),
            "the flux formatter would rewrite `{id}`"
        );

        let module = flux_lang::program::Module::parse_str(&emitted)
            .unwrap_or_else(|error| panic!("`{id}` does not load: {error}"));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{id}` is not a program"));
        assert_eq!(
            program.ops.len(),
            1,
            "one operation is one declaration; `{id}` loaded {}",
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, *id);
    }
}
