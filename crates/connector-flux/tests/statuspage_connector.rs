//! Atlassian Statuspage (C-181), and the three things this connector exists to establish.
//!
//! # 1. `OAuth` is a scheme *word*, not OAuth2 — and it is the first one to ship
//!
//! Statuspage authenticates with `Authorization: OAuth <api_key>`. The word `OAuth` is where
//! Okta's `SSWS` and PagerDuty's `Token token=` sit, and it means nothing more than they do: it is
//! public API syntax the vendor requires in front of a static key a human pasted in. There is no
//! authorization server, no grant, no refresh and no scope anywhere in this connector, so
//! **`[auth.oauth2]` is deliberately absent** and
//! [`the_scheme_word_oauth_is_a_prefix_and_not_an_oauth2_grant`] pins that absence. Declaring one
//! because the header says `OAuth` would tell flux to run an effectful acquisition against an
//! endpoint that does not exist.
//!
//! [C-184](../../../docs/stories/C-184-auth-scheme-prefix-axis.md) built the axis this needs, and
//! `crates/connector-spec/tests/auth_prefix.rs::a_scheme_word_that_is_not_oauth2_is_still_just_a_prefix`
//! pins the spelling at the model. **This file is the first time it ships**: every `header`
//! credential in the catalogue before Statuspage omitted `prefix` (Figma's `X-Figma-Token`,
//! GitLab's `PRIVATE-TOKEN`, LaunchDarkly's bare `Authorization`), so this is the connector that
//! carries a non-empty prefix into committed artifacts.
//!
//! # 2. The page id folds into `base_url`, and this is *not* the C-187 gap
//!
//! Every operation Statuspage publishes on this surface sits under one page:
//! `https://api.statuspage.io/v1/pages/{page_id}/…`. That reads at first like the path-segment
//! binding [C-187](../../../docs/stories/C-187-config-cannot-pin-a-request-component.md) measured
//! as out of reach — `ConfigField::binds` reaches `base_url` and nothing else. It is not, for
//! exactly the reason `providers/docusign.toml:117` already ships: the id sits at a *prefix* of
//! every path, so it folds into `base_url` as a `{page_id}` template variable bound by one
//! `[[config]]` field, the same mechanism Salesforce's `{instance}` uses.
//!
//! **The cost is real and is recorded rather than worked around.** Two of Statuspage's own
//! endpoints — `GET /v1/pages` and `GET /v1/pages/{page_id}` — sit *above* this base URL and are
//! therefore unreachable from this connector; DocuSign hit the identical wall at its own
//! `/accounts/{account_id}` tail and chose a sibling read for `verify` for the same reason. And an
//! account administering several pages needs one installation per page. C-187 remains the right
//! story for the general problem: had the page id sat mid-path or in a query string, no spelling
//! would exist at all.
//!
//! # 3. "Publicly visible" is not expressible, and this file does not imply that it is
//!
//! Creating a Statuspage incident publishes it to a public web page and — when
//! `deliver_notifications` is true — emails and texts every subscriber. The story asks for that to
//! be declared "as external-facing, not as an ordinary create", and **there is no field that can
//! say so.** `effects` is not authorable at all: `crates/connector-flux/src/op.rs:616` hardcodes
//! `effects: vec![from_tag("network")?]` for every generated op, which is the measurement C-155
//! recorded. `Risk` has four values — `Low`, `Medium`, `High`, `Destructive` — and none of them
//! means "external-facing".
//!
//! So the honest declaration is `risk = "high"`, the same value this repository already gives
//! `github-issue-create` and `launchdarkly-flag-toggle`, and
//! [`the_public_writes_are_high_risk_and_the_scale_cannot_say_why`] asserts it *together with* the
//! asymmetry the scale cannot carry: the incident itself is reversible — delete it, or resolve it —
//! but the subscriber email and SMS it dispatched are not. No `effects` key is invented here.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::config::template_variables;
use connector_spec::{AuthScheme, Binding, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

const PROVIDER: &str = "statuspage";

const CREDENTIAL: &str = "statuspage.api_key";
const KEY_ENV: &str = "STATUSPAGE_API_KEY";

/// The literal scheme word Statuspage requires, trailing space included. The space is not
/// decoration: the host appends the credential directly, so a prefix ending in an alphanumeric is a
/// load error since `3457581` hardened C-184's guard.
const SCHEME_WORD: &str = "OAuth ";

/// The curated operations, in the order `providers/statuspage.toml` declares them — the five the
/// story names and no more.
const OPERATIONS: &[&str] = &[
    "statuspage-incident-list",
    "statuspage-incident-get",
    "statuspage-incident-create",
    "statuspage-incident-update",
    "statuspage-component-list",
];

/// The two writes. Both publish to a page the general public reads.
const PUBLIC_WRITES: &[&str] = &["statuspage-incident-create", "statuspage-incident-update"];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn provider_toml_text() -> String {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    let source = provider_toml_text();
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

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

fn operation<'a>(connector: &'a Connector, id: &str) -> &'a connector_spec::Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("`{id}` is declared"))
}

/// **The load-bearing assertion.** `OAuth ` is a literal prefix on a header placement, and this
/// connector runs no OAuth2 grant — the word in the header is the vendor's syntax, not a protocol
/// this connector participates in.
#[test]
fn the_scheme_word_oauth_is_a_prefix_and_not_an_oauth2_grant() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Atlassian Statuspage");

    assert_eq!(
        connector.auth.len(),
        1,
        "statuspage authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("statuspage declares `{CREDENTIAL}`"));

    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: "Authorization".to_string(),
            prefix: SCHEME_WORD.to_string(),
        },
        "Statuspage sends `Authorization: OAuth <key>`; `bearer` would send `Bearer <key>` and \
         fail closed with 401 on every call, which is C-107's and C-161's recorded trap"
    );
    assert_eq!(method.env, [KEY_ENV]);

    // The trap this whole connector is named for. `OAuth` is a scheme word; there is no
    // authorization server, no grant, no refresh and no scope here.
    assert!(
        method.oauth2.is_none(),
        "`OAuth` in the header is a literal scheme word, not OAuth2. An `[auth.oauth2]` block \
         would tell the host to run an effectful grant against an endpoint this connector never \
         names"
    );

    // The trailing space is what keeps the prefix and the secret apart. Since `3457581` a prefix
    // ending in an alphanumeric is a load error, so `"OAuth"` would not even reach here — this
    // asserts the shipped file spells the separator rather than relying on the guard to notice.
    assert!(
        SCHEME_WORD.ends_with(' '),
        "the scheme word must end in a separator; the host appends the credential directly"
    );
    let AuthScheme::Header { prefix, .. } = &method.scheme else {
        unreachable!("asserted above")
    };
    assert!(
        !prefix
            .chars()
            .next_back()
            .is_some_and(|last| last.is_ascii_alphanumeric()),
        "a prefix ending in an alphanumeric would travel glued to the secret: {prefix:?}"
    );

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
            "operation `{}` has {} auth alternatives; statuspage is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the API key",
            operation.id
        );
    }
}

/// The page id is a prefix of every path, so it folds into `base_url` as one bound template
/// variable — the `providers/docusign.toml:117` mechanism, not the C-187 gap. The *cost* of the
/// fold is asserted here too, because it is a real limitation and not a free win: no operation may
/// reach above the page.
#[test]
fn the_page_id_folds_into_the_base_url_as_one_bound_variable() {
    let connector = load();

    assert_eq!(
        connector.base_url, "https://api.statuspage.io/v1/pages/{page_id}",
        "every curated operation sits under one page, so the id is a base-URL prefix"
    );

    let variables = template_variables(&connector.base_url);
    assert_eq!(
        variables,
        ["page_id"],
        "exactly one template variable — the page this connection administers"
    );

    let page_field = connector
        .config
        .iter()
        .find(|field| field.binds == "endpoint.page_id")
        .expect("no `[[config]]` field binds `endpoint.page_id`; the base URL template is unbound");
    assert_eq!(
        page_field.binding(),
        Some(Binding::Endpoint {
            variable: "page_id"
        })
    );
    assert!(
        !page_field.secret,
        "an `endpoint` binding is never a secret — a page id appears in the page's own public URL"
    );

    // The cost, asserted rather than merely written down: nothing here can reach `GET /v1/pages` or
    // `GET /v1/pages/{page_id}`, because both sit above this base URL. A connector that grew one
    // would have to spell `..` in a path, and this test is what would catch the attempt.
    for operation in &connector.operations {
        assert!(
            !operation.path.contains(".."),
            "operation `{}` tries to climb above the page its base URL pins: {:?}",
            operation.id,
            operation.path
        );
        assert!(
            !operation.path.contains("{page_id}"),
            "operation `{}` re-declares the base-URL variable in its own path, which would be a \
             second, contradictable source for it: {:?}",
            operation.id,
            operation.path
        );
    }

    // Every field is renderable, and `secret` agrees with `binds` in both directions.
    for field in &connector.config {
        assert!(
            !field.label.is_empty() && !field.help.is_empty(),
            "config field `{}` must be renderable",
            field.name
        );
    }
    let credential_field = connector
        .config
        .iter()
        .find(|field| field.binds == format!("credential.{CREDENTIAL}"))
        .unwrap_or_else(|| panic!("no `[[config]]` field binds `credential.{CREDENTIAL}`"));
    assert!(
        credential_field.secret,
        "the API key binds a credential and must be declared secret"
    );
    assert!(
        credential_field.example.is_none(),
        "no realistic example on a secret field — a plausible token has tripped push protection \
         and blocked a release before"
    );

    assert_eq!(
        connector.config.len(),
        2,
        "statuspage asks for exactly two things: the page id and the API key"
    );

    // The template reaches every emitted module verbatim, unresolved.
    for (id, text) in emitted(&connector) {
        assert!(
            text.contains(r#"base = "https://api.statuspage.io/v1/pages/{page_id}""#),
            "`{id}` does not carry the unbound page template:\n{text}"
        );
    }
}

/// **The declaration the risk scale cannot make, made as precisely as the scale allows.**
///
/// Both writes publish to a page the general public reads, and a create with
/// `deliver_notifications = true` dispatches email and SMS to every subscriber. Nothing in this
/// model can say "external-facing": `effects` is hardcoded to `["network"]` at
/// `crates/connector-flux/src/op.rs:616` and is not authorable, and `Risk` has four values none of
/// which means it. `high` is the honest choice — it is what puts the call behind flux's approval
/// gate — and the asymmetry it cannot carry is stated in the operation's own `description`, which
/// is the one string a model actually reads.
#[test]
fn the_public_writes_are_high_risk_and_the_scale_cannot_say_why() {
    let connector = load();

    for id in PUBLIC_WRITES {
        let write = operation(&connector, id);
        assert_eq!(
            write.risk,
            Risk::High,
            "`{id}` publishes to a public page; `medium` would let an agent post to it with no \
             human seeing it first"
        );
        assert_eq!(
            write.idempotency,
            Idempotency::NonIdempotent,
            "`{id}` is a POST or PATCH, which RFC 9110 §9.2.2 does not make idempotent"
        );

        // The scale stops at `high`, so the part it cannot carry has to be in the prose the model
        // reads: the incident is reversible, the notification it sent is not.
        let described = write.description.to_lowercase();
        assert!(
            described.contains("subscriber"),
            "`{id}`'s description must name the subscribers it notifies, because no `effects` \
             field can: {:?}",
            write.description
        );
        assert!(
            described.contains("public"),
            "`{id}`'s description must say the page is public, because no `risk` value can: {:?}",
            write.description
        );
    }

    // No `effects` key was invented to say what the model cannot express. The provider TOML is the
    // place an author would reach for one, so this checks the file's own text.
    let toml_text = provider_toml_text();
    assert!(
        !toml_text.contains("effects ="),
        "providers/statuspage.toml declares an `effects` key; there is no such field, and \
         inventing one would imply an expressiveness this pipeline does not have"
    );

    // Every read is low risk and idempotent; no write claims low risk.
    for operation in &connector.operations {
        match operation.method {
            HttpMethod::Get => {
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
            HttpMethod::Post | HttpMethod::Patch => {
                assert_ne!(
                    operation.risk,
                    Risk::Low,
                    "operation `{}` is a write declared low risk",
                    operation.id
                );
            }
            other => panic!(
                "operation `{}` uses method {other:?} this connector does not curate",
                operation.id
            ),
        }
    }
}

/// **`deliver_notifications` is required on both writes, and that is the closest this model gets to
/// declaring the effect it cannot name.**
///
/// Under C-56 an optional body field travels as an explicit `null`, so every declared body field
/// has to be required anyway. Making *this* one required is therefore free, and it buys the thing
/// the risk scale could not: a caller cannot post to a public status page without stating, in the
/// call itself, whether every subscriber gets an email and a text about it.
#[test]
fn deliver_notifications_is_a_required_choice_on_every_write() {
    let connector = load();

    for id in PUBLIC_WRITES {
        let write = operation(&connector, id);

        let field = write
            .params
            .body
            .iter()
            .find(|param| param.name == "deliver_notifications")
            .unwrap_or_else(|| {
                panic!(
                    "`{id}` does not declare `deliver_notifications`; a caller could publish to a \
                     public page without choosing whether to notify every subscriber"
                )
            });
        assert!(
            field.required,
            "`{id}`'s `deliver_notifications` must be required — the choice is the point, and \
             under C-56 an optional body field travels as an explicit `null` regardless"
        );
        assert_eq!(
            field.schema.get("type").and_then(|value| value.as_str()),
            Some("boolean")
        );
        assert_eq!(
            field.wire.as_deref(),
            Some("incident.deliver_notifications"),
            "Statuspage nests every incident body field under an `incident` object"
        );

        // C-56 in full: every body field on these writes is required, so none of them travels as
        // an explicit `null` and clears a value the caller never mentioned.
        for param in &write.params.body {
            assert!(
                param.required,
                "`{id}`'s body field `{}` is optional, so it would travel as an explicit `null` \
                 (C-56) and overwrite whatever the incident already carries",
                param.name
            );
            assert!(
                param
                    .wire
                    .as_deref()
                    .is_some_and(|wire| wire.starts_with("incident.")),
                "`{id}`'s body field `{}` is not nested under `incident`",
                param.name
            );
        }
    }
}

/// The `verify` operation is a read, takes no argument beyond the configured page, and is
/// unattended-safe — it is the "Test connection" button on a settings page.
#[test]
fn verify_is_an_unattended_read_that_needs_no_argument() {
    let connector = load();

    let verify_id = connector
        .verify
        .as_deref()
        .expect("statuspage names a `verify` operation");
    assert_eq!(verify_id, "statuspage-component-list");

    let verify = operation(&connector, verify_id);
    assert_eq!(verify.method, HttpMethod::Get);
    assert_eq!(verify.risk, Risk::Low);
    assert!(
        verify.params.path.is_empty()
            && verify.params.body.is_empty()
            && verify.params.query.is_empty(),
        "`{verify_id}` must take no argument at all; a settings page calls it with nothing but the \
         configuration"
    );
}

/// Nothing here reaches a query string, so the missing percent-encoder (C-30,
/// `zendesk-ticket-search`'s own defect) cannot corrupt a Statuspage request. Statuspage's own
/// `page`/`per_page` paging and its incident `q` search are excluded — the paging parameters
/// because this file has no verified account of their bounds, and `q` because it is exactly the
/// free-text query value C-30 measures.
#[test]
fn no_operation_reaches_a_query_string_at_all() {
    let connector = load();

    let with_query: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| !operation.params.query.is_empty())
        .map(|operation| operation.id.as_str())
        .collect();
    assert!(
        with_query.is_empty(),
        "these operations declare a query parameter with no percent-encoder to protect it \
         (C-30): {with_query:?}"
    );
}

/// No credential, and no credential's variable name or scheme word, reaches a generated module.
/// The prefix is the new surface here: it is connector data, but it belongs in the manifest and the
/// catalogue, never in emitted Flux, which names a credential and nothing more.
#[test]
fn no_statuspage_module_carries_a_credential_a_header_or_the_scheme_word() {
    let connector = load();

    for (id, text) in emitted(&connector) {
        for forbidden in [
            KEY_ENV,
            CREDENTIAL,
            "$secret",
            "Authorization",
            "OAuth",
            "Bearer",
        ] {
            assert!(
                !text.contains(forbidden),
                "`{id}` names `{forbidden}` in generated Flux; a generated module carries no \
                 credential, no credential reference and no placement syntax (C-10, AGENTS.md):\n\
                 {text}"
            );
        }
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical,
/// and loads as exactly one exposed composite op.
#[test]
fn every_statuspage_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
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
