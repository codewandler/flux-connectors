//! Resend (C-222) is the epic's probe for the **floor**: how small a good connector can be.
//!
//! Every other provider story in this wave was selected because it forces the model to confront
//! something new — a query-placed credential, a two-token partition, a service split. This one was
//! selected because it forces *nothing*. A plain bearer token, one fixed host, no service split, no
//! pagination puzzle, no template variable in the base URL. What is left when all of that is gone is
//! the irreducible part of a connector, and that is what this file measures.
//!
//! Four findings, each pinned below:
//!
//! 1. **The whole credential surface is one bearer token**, and nothing about it is conditional:
//!    one `[[auth]]` entry, one alternative naming it, and every operation inheriting that default.
//! 2. **The connector declares no `[[config]]`, no `[[services]]`, no `[[events]]`, no
//!    `[[channels]]` and no `[[graphs]]` — and it loads, emits and renders anyway.** The story asked
//!    whether the empty configuration surface actually works end to end, on the suspicion that every
//!    shipped example carries at least one field and the empty case may simply be untested. It is
//!    not untested: this test also names the other shipped providers that declare none, so the
//!    finding is measured rather than asserted.
//! 3. **The curated set is four operations, and the emitter's constraints are what bound it.** Not
//!    one query parameter and not one optional body field: query values are interpolated verbatim
//!    (`crates/connector-flux/src/op.rs:138-143`) and an unset optional body field travels as an
//!    explicit `null` (`body_tree`, `op.rs:1062-1103`), so a connector that wants neither hazard
//!    declares neither shape. This test pins both, on the declaration *and* on the emitted Flux.
//! 4. **Nothing token-shaped appears anywhere in the file.** Resend prefixes every API key it issues
//!    with `re_`, which is exactly the shape a secret scanner matches, and a placeholder shaped like
//!    a real token has blocked a release in this repository before.
//! 5. **The one thing the floor did force: a `User-Agent`.** Resend rejects a request without one
//!    with `403`, valid key and all, and nothing in this repository binds an HTTP implementation
//!    that could be checked for supplying it. So it is declared as a constant header and asserted
//!    onto every emitted request here — this is the single fact about this connector that its
//!    endpoints do not carry, and therefore the one a spec ingest would not have found either.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod, Idempotency, Risk};

/// The provider under test.
const PROVIDER: &str = "resend";

const CREDENTIAL: &str = "resend.api_key";
/// A variable *name*; no credential value appears in this repository.
const CREDENTIAL_ENV: &str = "RESEND_API_KEY";

const BASE_URL: &str = "https://api.resend.com";
const AUTHORITY: &str = "com.resend.api";

/// The verification read — argument-free, so a settings page can run it unattended.
const VERIFY: &str = "resend-domain-list";

/// The four curated operations, in the order `providers/resend.toml` declares them.
///
/// **Four is the answer, not a placeholder for more.** Resend documents batch send, scheduled send
/// with update and cancel, API keys, audiences, contacts, broadcasts and webhooks besides; the
/// provider file names what it left out and why.
const OPERATIONS: &[&str] = &[
    "resend-email-send",
    "resend-email-get",
    "resend-domain-list",
    "resend-domain-get",
];

/// The header Resend rejects a request for omitting, and the value this connector sends.
const REQUIRED_HEADER: &str = "User-Agent";
const REQUIRED_HEADER_VALUE: &str = "flux-connectors";

/// The prefix Resend puts on every API key it issues, and therefore the byte sequence that must
/// never appear in this connector's source — in an `example`, in a `help` string, or in a response
/// schema's illustration.
const KEY_PREFIX: &str = "re_";

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn source_of(id: &str) -> String {
    let path = providers_dir().join(format!("{id}.toml"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-222 ships the Resend connector",
            path.display()
        )
    })
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    load_provider(PROVIDER)
}

fn load_provider(id: &str) -> Connector {
    provider::load(&format!("providers/{id}.toml"), &source_of(id))
        .unwrap_or_else(|error| panic!("providers/{id}.toml does not load: {error}"))
        .connector
}

fn shipped() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(providers_dir())
        .expect("providers/ is readable")
        .filter_map(|entry| {
            let path = entry.expect("a readable directory entry").path();
            (path.extension()? == "toml").then(|| {
                path.file_stem()
                    .expect("a .toml file has a stem")
                    .to_string_lossy()
                    .into_owned()
            })
        })
        .collect();
    names.sort();
    names
}

/// **Finding 1: the whole credential surface is one bearer token, unconditionally.**
///
/// One `[[auth]]` entry, one requirement set naming it, and no operation overriding it. There is no
/// second credential to pair it with, no `user_env` to join it to, no OAuth grant to run for it and
/// no prefix to spell — `AuthScheme::Bearer` is the preset that carries all of that already. This is
/// the smallest authentication a connector in this repository can declare, and it is the reason the
/// rest of the file is as short as it is.
#[test]
fn the_whole_credential_surface_is_one_bearer_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Resend");
    assert_eq!(connector.base_url, BASE_URL);
    assert_eq!(connector.authority.as_deref(), Some(AUTHORITY));

    // The base URL is finished, not a template. Nothing to bind means nothing a human has to type
    // before the connector can address the vendor at all — half of why there is no config surface.
    assert!(
        !connector.base_url.contains('{'),
        "one fixed host: a `{{variable}}` here would need a config field to bind it"
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "one credential, and Resend documents no other mechanism"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("resend declares `{CREDENTIAL}`"));
    assert_eq!(method.scheme, AuthScheme::Bearer);
    assert_eq!(method.env, [CREDENTIAL_ENV]);

    assert_eq!(
        connector.default_auth.len(),
        1,
        "one alternative — there is no second way to authenticate a Resend request"
    );
    let mechanism: Vec<&str> = connector.default_auth[0]
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(mechanism, [CREDENTIAL]);

    for id in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("resend declares `{id}`"));
        let effective: Vec<Vec<&str>> = connector
            .effective_auth(operation)
            .iter()
            .map(|requirement| requirement.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(
            effective,
            vec![vec![CREDENTIAL]],
            "every operation carries the one credential; none overrides the default"
        );
    }
}

/// **Finding 5: every request carries a `User-Agent`, because Resend `403`s the ones that do not.**
///
/// The floor probe's one surprise. Resend's introduction states it plainly — *"All API requests must
/// include a `User-Agent` header … Requests without this header will be rejected with a `403` status
/// code"* — and adds that SDKs supply one but direct HTTP callers must set it themselves. This
/// pipeline emits direct `http.request` calls, and nothing here binds an implementation whose
/// defaults could be inspected (`AGENTS.md`, Intentional gaps: `flux-web` is not in `Cargo.lock`),
/// so the header is declared rather than assumed.
///
/// Asserted on the emitted Flux, not only on the declaration, because that is where the omission
/// would actually bite: a connector that declared the header and dropped it during emission would
/// still load, still build, still ship, and still `403` on the first real call.
#[test]
fn every_emitted_request_carries_the_user_agent_resend_demands() {
    let connector = load();

    for operation in &connector.operations {
        assert_eq!(
            operation.params.const_headers.get(REQUIRED_HEADER),
            Some(&REQUIRED_HEADER_VALUE.to_string()),
            "{}: Resend rejects a request without a `{REQUIRED_HEADER}` with 403, valid key and all",
            operation.id
        );

        // The emitter binds a constant header to a symbol and names the symbol in the request's
        // header record, so both halves are checked: a binding with no reference would be dead, and
        // a reference with no binding would not parse.
        let flux = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{} does not emit: {error}", operation.id));
        let symbol = REQUIRED_HEADER.replace('-', "_");
        assert!(
            flux.contains(&format!("{symbol} = \"{REQUIRED_HEADER_VALUE}\""))
                && flux.contains(&format!("\"{REQUIRED_HEADER}\": {symbol}")),
            "{} emits no `{REQUIRED_HEADER}` header:\n{flux}",
            operation.id
        );
    }
}

/// **Finding 2, and the question the story asked: a connector with no `[[config]]` surface works end
/// to end — and the empty case was never untested.**
///
/// The story's premise was that "every shipped example has at least one config field, so the empty
/// case may be untested". Measured here rather than assumed: this test names every shipped provider
/// declaring no configuration field, and asserts Resend is *not* the first. The empty surface has
/// been exercised by the shipped catalogue since before this connector existed, which is a finding
/// about the premise and not about Resend.
///
/// What Resend adds is the *complete* floor — no config, no services, no events, no channels, no
/// graphs — with a connector that still loads through the real loader and still emits every
/// operation. Everything a human must supply here is the one credential, and a credential is
/// declared in `[[auth]]`, not in `[[config]]`: a `[[config]]` field binding it would only restate
/// what `[[auth]]` already says, and `AGENTS.md`'s configuration contract is that a connector "asks
/// for everything it needs and nothing it cannot use".
#[test]
fn no_configuration_surface_is_declared_and_the_connector_still_holds() {
    let connector = load();

    assert!(
        connector.config.is_empty(),
        "the floor declares no configuration field: the only thing a human supplies is the \
         credential, which `[[auth]]` already names"
    );
    assert!(
        connector.services.is_empty(),
        "one API surface, so the implicit `default` service is the whole connector"
    );
    assert!(
        connector.events.is_empty() && connector.channels.is_empty(),
        "no inbound half: Resend does publish webhooks, and declaring one means also declaring how \
         it is registered and verified, which is its own story"
    );
    assert!(connector.graphs.is_empty(), "no composed flow");

    // Every operation still emits, so the empty surface is not merely accepted by the loader — it
    // reaches Flux.
    for operation in &connector.operations {
        emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{} does not emit: {error}", operation.id));
    }

    // The premise, measured. A zero-config connector is not new; Resend is one of several.
    let mut without_config: Vec<String> = shipped()
        .into_iter()
        .filter(|id| load_provider(id).config.is_empty())
        .collect();
    without_config.sort();
    assert!(
        without_config.contains(&PROVIDER.to_string()),
        "resend declares no config field"
    );
    assert!(
        without_config.len() > 1,
        "the story's premise was that the empty `[[config]]` case may be untested. If resend is \
         genuinely the only zero-config provider, that premise holds and this connector is the \
         first exercise of it — say so rather than deleting this assertion. Measured: {without_config:?}"
    );
}

/// **Finding 3: the curated set is four operations, and nothing in it enters an emitter gap.**
///
/// Two gaps bound this connector, and both are the emitter's rather than the vendor's:
///
/// - **A query value is interpolated verbatim** (`crates/connector-flux/src/op.rs:138-143`), the
///   standing `zendesk-ticket-search` gap in `AGENTS.md`. So no operation declares a query
///   parameter, and no emitted URL carries a `?`.
/// - **An unset optional body field travels as an explicit `null`.** `body_tree` places every
///   declared field into the payload record unconditionally (`op.rs:1062-1103`), and there is no
///   `when` guard on the body side the way there is on the query side. Whether Resend tolerates
///   `{"cc": null}` is not documented either way, so every body field this connector declares is
///   `required = true` — the answer `providers/openai.toml` reached first, for the same reason.
///
/// The positive half matters as much as the negative: a connector that declared *nothing* would
/// satisfy both assertions vacuously, so the send operation's real body is pinned too.
#[test]
fn the_curated_set_declares_no_query_parameter_and_no_optional_body_field() {
    let connector = load();

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        declared, OPERATIONS,
        "four curated operations, in declaration order — not padded to look substantial"
    );

    for operation in &connector.operations {
        assert!(
            operation.params.query.is_empty(),
            "{} declares a query parameter, and every query value this emitter writes is \
             interpolated verbatim",
            operation.id
        );
        for param in &operation.params.body {
            assert!(
                param.required,
                "{}: body field `{}` is optional, and an unset optional body field travels as an \
                 explicit null rather than being omitted",
                operation.id, param.name
            );
        }

        let flux = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{} does not emit: {error}", operation.id));
        assert!(
            !flux.contains('?'),
            "{} emits a `?` into its URL:\n{flux}",
            operation.id
        );
        assert!(
            !flux.contains("sep = "),
            "{} emits the optional-query-parameter machinery:\n{flux}",
            operation.id
        );
    }

    // The positive half: the send really does carry an email, and it carries it in a JSON body.
    let send = connector
        .operation("resend-email-send")
        .expect("the curated set includes the send");
    let body: Vec<&str> = send
        .params
        .body
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    assert_eq!(
        body,
        ["from", "to", "subject", "html"],
        "the four fields Resend requires to accept a message, every one of them always sent"
    );
    let flux = emit_operation(&connector, send).expect("the send emits");
    assert!(
        flux.contains("content_type = \"application/json\""),
        "the send carries a JSON body:\n{flux}"
    );
}

/// **Every operation carries the metadata a model reads and an approval gate acts on.**
///
/// `risk` and `idempotency` are not decoration: flux's approval gate reads `risk`, so a send
/// declaring `low` would be waved through unattended, and a `POST` declaring itself `idempotent`
/// would make a `retry` around it send the message three times. The emitter refuses both outright
/// (`check_write_metadata`, `op.rs:708-730`), so this test pins the *shape* of the declarations
/// rather than re-deriving the refusal — plus the two things the emitter does not check: that every
/// operation states a description a model can act on, and that every one declares the response shape
/// Resend documents.
#[test]
fn every_operation_declares_its_risk_its_idempotency_and_its_response_shape() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.description.len() > 40,
            "{}: `description` is the tool contract a model reads, not a label",
            operation.id
        );
        assert!(
            operation.response_schema.is_some(),
            "{}: Resend documents an example response for every operation this connector selects, \
             so an absent shape here would be an omission rather than an honest silence",
            operation.id
        );

        let mutates = matches!(
            operation.method,
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete
        );
        if mutates {
            assert_ne!(
                operation.risk,
                Risk::Low,
                "{}: a write is never `low`",
                operation.id
            );
            assert_eq!(
                operation.idempotency,
                Idempotency::NonIdempotent,
                "{}: both writes here are `POST`, and Resend documents no idempotency key for \
                 either",
                operation.id
            );
        } else {
            assert_eq!(operation.risk, Risk::Low);
            assert_eq!(operation.idempotency, Idempotency::Idempotent);
        }
    }

    // The one operation whose blast radius is the point: a sent email cannot be recalled.
    let send = connector
        .operation("resend-email-send")
        .expect("the curated set includes the send");
    assert_eq!(
        send.risk,
        Risk::High,
        "a message that has left the building cannot be unsent — the same declaration \
         `providers/postmark.toml` records for its own send"
    );
}

/// **`verify` is a read that runs unattended.**
///
/// A "Test connection" button is pressed whenever someone opens a settings page, so it must be a
/// read — which the loader checks — *and* it must need no argument, which the loader does not check
/// and a connector can still get wrong. `GET /domains` is the choice here rather than `GET
/// /api-keys`: both are argument-free reads, and only one of them answers with a list of the
/// account's credentials.
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

/// **Finding 4: nothing token-shaped is in the file, and nothing token-shaped reaches the emitted
/// Flux.**
///
/// Resend prefixes every key it issues with `re_`, so an illustrative `re_…` placeholder would be
/// both a lie and a secret-scanner match. A token-shaped placeholder has blocked a release in this
/// repository before, which is why this is a test rather than a review habit.
///
/// The emitted side is checked too, and for a different reason: generated Flux names a credential
/// and nothing more (`AGENTS.md`, the authentication contract), so neither the value nor even the
/// environment variable's name belongs in a `.flux` module.
#[test]
fn no_token_shaped_value_appears_in_the_source_or_in_the_emitted_flux() {
    let source = source_of(PROVIDER);
    for (number, line) in source.lines().enumerate() {
        // Naming the prefix in prose is how the hazard gets recorded; what must never appear is a
        // value shaped like one Resend actually issued.
        assert!(
            !looks_issued(line),
            "providers/{PROVIDER}.toml:{}: a value shaped like an issued Resend key",
            number + 1
        );
    }

    let connector = load();
    for operation in &connector.operations {
        let flux = emit_operation(&connector, operation).expect("shipped operations emit");
        assert!(
            !flux.contains(CREDENTIAL_ENV) && !flux.contains(KEY_PREFIX),
            "{} carries credential material into generated Flux",
            operation.id
        );
    }
}

/// Whether a line carries something shaped like an *issued* key rather than a mention of the prefix:
/// `re_` followed by enough of an alphanumeric run to read as a real value.
fn looks_issued(line: &str) -> bool {
    line.match_indices(KEY_PREFIX).any(|(at, _)| {
        line[at + KEY_PREFIX.len()..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .count()
            >= 8
    })
}
