//! Okta (C-161) is the epic's probe for an Authorization scheme word this model had never been asked
//! for. Fifteen shipped providers were `bearer` and two `basic`; Okta authenticates with
//! `Authorization: SSWS <apiToken>` — a **custom scheme word**, not a placement the model had met.
//!
//! This file began life as a probe with no connector behind it, because the connector could not
//! authenticate honestly with the `AuthScheme` of the day. It is now a per-provider contract test in
//! the shape of every sibling in this directory, loading the `providers/okta.toml` that ships. The
//! probe's findings are kept rather than deleted, because they are the measurement that produced the
//! axis this connector is built on:
//!
//! 1. **[`AuthScheme`] is a closed, five-member enum**, and a vendor's scheme word is *data*, not a
//!    variant. Naming Okta's own word (`ssws`) directly is still refused at deserialization — it is
//!    not `bearer`, `basic`, `header`, `query` or `signing`. This is why C-184's answer was a field
//!    on `Header` and not a sixth variant.
//! 2. **`Header` had no field to carry the word on.** `docs/designs/unified-auth.md` proposed exactly
//!    that field — a `prefix` on header placement, "the single highest-value element of this whole
//!    design" — and it was never implemented. [C-184](../../../docs/stories/C-184-auth-scheme-prefix-axis.md)
//!    built it, and this story is what it unblocked. The test that recorded finding 2 asserted the
//!    refusal; it now asserts the connector, which is the same seam read from the other side.
//! 3. **A bare `header` placement aimed at `Authorization` loads, and that is the trap.**
//!    `AuthScheme::Header` does not know or care what header name it is given, so omitting the prefix
//!    is legal — and *correct* for LaunchDarkly and ClickUp, whose whole header value is the token.
//!    Applied to Okta it would send `Authorization: <token>` with the literal word `SSWS` simply
//!    missing, which the vendor answers `401`. The escape is to declare the prefix, not to bake
//!    `"SSWS "` into the credential value, which AGENTS.md refuses outright ("no credential value
//!    enters provider TOML, generated Flux, a manifest, the public catalogue, or the lockfile").
//!
//! What this file asserts about the shipped connector, beyond that it parses:
//!
//! - the credential is `AuthScheme::Header { name: "Authorization", prefix: "SSWS " }`, **trailing
//!   space included**, and round-trips through `toml` to exactly that;
//! - dropping the trailing space is refused by the loader rather than shipped — the separator rule
//!   commit `3457581` added, which is the one guard standing between `SSWS <token>` and the
//!   `SSWS<token>` no vendor accepts;
//! - the scheme word reaches **no generated Flux**. It is connector data the host applies at the
//!   placement seam, and generated Flux names a credential and nothing more;
//! - `okta-user-deactivate` is declared `destructive` and `non_idempotent`, and it is the only
//!   operation above `low`;
//! - no curated operation exposes Okta's free-text `q`/`filter`/`search` or its `after` cursor — both
//!   are excluded deliberately, and `providers/okta.toml`'s header comment says why.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider under test. Named once so the file reads as being about Okta rather than a string.
const PROVIDER: &str = "okta";

/// The credential the connector declares, the header it travels in, the literal word in front of it,
/// and the variable it resolves from. All four are public contract — an operator sets the variable,
/// a manifest names the credential, Okta reads the header — so they are pinned here rather than left
/// to whatever the provider file happens to say.
const CREDENTIAL: &str = "okta.api_token";
/// See [`CREDENTIAL`]. Okta authenticates on the standard `Authorization` header.
const AUTH_HEADER: &str = "Authorization";
/// See [`CREDENTIAL`]. **The trailing space is part of the literal**, not a separator the host
/// inserts — see [`the_scheme_word_ends_in_the_separator_the_wire_form_requires`].
const SCHEME_WORD: &str = "SSWS ";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "OKTA_API_TOKEN";

/// Every Okta org has its own host (`acme.okta.com`, `acme.okta-emea.com`, or a customised domain),
/// so the host is a bound `{domain}` the way zendesk's `{subdomain}` is — not a fixed literal.
const BASE_URL: &str = "https://{domain}/api/v1";

/// The "Test connection" read, and the connector's declared `verify`.
const VERIFY_OPERATION: &str = "okta-user-list";
/// The one write, and the only operation above `low` risk.
const DEACTIVATE_OPERATION: &str = "okta-user-deactivate";

/// The five curated operations, in the order `providers/okta.toml` declares them.
const OPERATIONS: &[&str] = &[
    VERIFY_OPERATION,
    "okta-user-get",
    "okta-group-list",
    "okta-user-group-list",
    DEACTIVATE_OPERATION,
];

/// Query parameter names this connector deliberately does not offer. `q`, `filter` and `search` are
/// Okta's free-text and SCIM-expression filters, which land in a URL this repository cannot
/// percent-encode (the C-30 gap AGENTS.md records under `zendesk-ticket-search`); `after` is Okta's
/// pagination cursor, which is only ever handed back in a `Link` **response header** that this model
/// has no way to surface to a caller, so exposing the parameter would offer a knob nobody can turn.
const EXCLUDED_QUERY_PARAMS: &[&str] = &["q", "filter", "search", "after"];

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
            "cannot read {} ({error}) — C-161 ships the Okta connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// Every operation's emitted module, paired with its id.
fn emitted() -> Vec<(String, String)> {
    let connector = load();
    connector
        .operations
        .iter()
        .map(|operation| {
            let flux = emit_operation(&connector, operation)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
            (operation.id.clone(), flux)
        })
        .collect()
}

/// A minimal, otherwise-valid provider fixture. Only the `[[auth]]` block's `scheme` line varies
/// across the cases below — everything else is held constant so a failure is about `scheme` and
/// nothing else. This is the probe's own fixture, kept because findings 1 and 3 are about what the
/// loader *refuses*, and a refusal cannot be measured against a file that ships.
fn fixture(auth_scheme_toml: &str) -> String {
    format!(
        r#"
id = "okta"
vendor = "Okta"
base_url = "https://acme.okta.com"

[[auth]]
name = "okta.api_token"
env = ["OKTA_API_TOKEN"]
{auth_scheme_toml}

[[operations]]
id = "okta-user-list"
method = "GET"
path = "/api/v1/users"
risk = "low"
idempotency = "idempotent"
description = "List users in the Okta org, for the probe fixture only"
"#
    )
}

/// **Finding 1, unchanged: `AuthScheme` is a closed enum and `ssws` is not one of its members.**
///
/// `crates/connector-spec/src/auth.rs` declares
/// `#[serde(rename_all = "snake_case", deny_unknown_fields)] pub enum AuthScheme { Bearer, Basic,
/// Header { name, prefix }, Query { name }, Signing }` — five variants naming a *placement*. C-184
/// did not open the enum, and this test is why it did not need to: a vendor's scheme word is data
/// that rides on `Header`, so naming it as a variant is still refused at deserialization rather than
/// silently coerced to a preset.
#[test]
fn an_arbitrary_scheme_word_is_not_a_variant_of_auth_scheme() {
    let source = fixture("scheme = \"ssws\"");
    let error = provider::load("providers/okta-probe.toml", &source)
        .expect_err("`ssws` is not one of AuthScheme's five variants and must be refused");
    let message = error.to_string();
    assert!(
        message.to_lowercase().contains("unknown variant"),
        "expected an unknown-variant error naming the closed enum, got: {message}"
    );
}

/// **Finding 3, unchanged: a bare `header` placement on `Authorization` still loads, and it is still
/// the trap.**
///
/// C-184 did not close this one, and could not: omitting `prefix` is *correct* for LaunchDarkly and
/// ClickUp, whose whole Authorization value is the token, so a connector that omits it is
/// indistinguishable at the model from one that forgot. Applied to Okta the wire form would be
/// `Authorization: <token>` with the word `SSWS` simply missing — a request the vendor rejects, and
/// the reason C-161 called this the trap rather than the gap. What the shipped connector does
/// instead is the test below.
#[test]
fn a_bare_header_placement_still_omits_the_scheme_word_it_does_not_declare() {
    let source = fixture("scheme = { header = { name = \"Authorization\" } }");
    let connector = provider::load("providers/okta-probe.toml", &source)
        .expect("a bare `header` placement on `Authorization` is legal — that is exactly the trap")
        .connector;
    let method = connector
        .auth_method(CREDENTIAL)
        .expect("the fixture declares it");
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: AUTH_HEADER.to_string(),
            prefix: String::new(),
        }
    );

    let round_tripped = toml::to_string(&method.scheme).expect("AuthScheme serializes");
    assert_eq!(
        round_tripped.trim(),
        "[header]\nname = \"Authorization\"",
        "an empty prefix must not reach the encoding — every provider authored before C-184 depends \
         on serializing exactly as it did"
    );
}

/// **The headline: the connector the probe refused to ship now ships, and it carries `SSWS `.**
///
/// This test is the inversion of the probe's `no_provider_toml_was_shipped_for_this_probe`, which
/// asserted `providers/okta.toml` did *not* exist and said in as many words that a future story
/// adding one must do so "only once `AuthScheme` (or an equivalent seam) can carry an arbitrary
/// prefix". C-184 built that seam; this is the story that used it.
///
/// The round trip through `toml` is the sharpest way to state the contract: two fields, `name` and
/// `prefix`, and the prefix is the vendor's public scheme word with **no credential value anywhere
/// near it** — the connector holds the word and the *name* of an environment variable, and nothing
/// that resolves to a secret.
#[test]
fn the_shipped_connector_carries_the_ssws_scheme_word_the_probe_could_not() {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    assert!(
        path.exists(),
        "providers/okta.toml does not exist. C-161's probe recorded a refusal to ship it until a \
         prefix axis existed; C-184 built one, so the connector is expected here"
    );

    let connector = load();
    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Okta");
    assert_eq!(connector.base_url, BASE_URL);
    assert_eq!(connector.verify.as_deref(), Some(VERIFY_OPERATION));

    assert_eq!(
        connector.auth.len(),
        1,
        "okta authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("okta declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: AUTH_HEADER.to_string(),
            prefix: SCHEME_WORD.to_string(),
        },
        "Okta sends `{AUTH_HEADER}: {SCHEME_WORD}<token>`. The scheme word is connector data on the \
         placement; the token is appended by the host and is never written in this repository"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a prefixed header credential has no user half"
    );

    let round_tripped = toml::to_string(&method.scheme).expect("AuthScheme serializes");
    assert_eq!(
        round_tripped.trim(),
        "[header]\nname = \"Authorization\"\nprefix = \"SSWS \"",
        "two fields and nothing else — the vendor's public scheme word and the header it rides on"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    // Every operation resolves to the one credential, whether it declares auth or inherits the
    // connector default, and none declares a caller-supplied `Authorization` header of its own —
    // that header is injected by the host, and a caller-facing parameter for it would let a caller
    // override both the credential and the scheme word in front of it.
    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; okta is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the API token",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares a caller-supplied header; `{AUTH_HEADER}` is injected by the \
             host and must never travel through the parameter surface",
            operation.id
        );
    }
}

/// **The trailing space is load-bearing, and the loader now enforces it.**
///
/// The host appends the credential to the prefix with nothing in between, so `"SSWS"` would compose
/// `Authorization: SSWS<token>` — a header Okta rejects, from a connector that looks correct in
/// review. When this file was a probe it said this was something "nothing can catch for the author".
/// Commit `3457581` made it catchable with a structural rule: a non-empty prefix must not end in an
/// alphanumeric, because a well-formed scheme word always ends in a separator.
///
/// Asserted from both sides — the shipped value ends in the separator, and the spelling without it is
/// refused with the clause that is actually about the separator, so a guard firing for some other
/// reason cannot satisfy this test.
#[test]
fn the_scheme_word_ends_in_the_separator_the_wire_form_requires() {
    assert!(
        SCHEME_WORD.ends_with(' '),
        "the space after `SSWS` is part of the literal, not something the host inserts"
    );

    let connector = load();
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("okta declares `{CREDENTIAL}`"));
    let AuthScheme::Header { prefix, .. } = &method.scheme else {
        panic!("okta's credential is a header placement");
    };
    assert!(
        !prefix
            .chars()
            .next_back()
            .expect("okta declares a non-empty prefix")
            .is_ascii_alphanumeric(),
        "the shipped prefix {prefix:?} ends in an alphanumeric, so the host would glue the token \
         straight onto it"
    );

    let error = provider::load(
        "providers/okta-probe.toml",
        &fixture("scheme = { header = { name = \"Authorization\", prefix = \"SSWS\" } }"),
    )
    .expect_err("a prefix without its trailing separator must be refused, not shipped");
    let message = error.to_string();
    assert!(
        message.contains("ending in an alphanumeric character"),
        "expected the separator-rule refusal, got: {message}"
    );
}

/// **The scheme word reaches no generated Flux.** The declaration proves the *placement* is right;
/// this proves the emitter does not quietly write the word into the module.
///
/// AGENTS.md's authentication contract is explicit: "Generated Flux names a credential and nothing
/// more. It must not add prefixes." `SSWS ` is exactly a prefix, so a connector that spells one is
/// the sharpest possible test of that rule — and the same assertion catches the other half, that the
/// credential's environment variable does not leak into a module either.
#[test]
fn no_emitted_operation_carries_the_scheme_word_or_the_credential_variable() {
    for (id, flux) in emitted() {
        assert!(
            !flux.contains("SSWS"),
            "`{id}` emits the literal scheme word `SSWS`, but a prefix is applied host-side at the \
             placement seam — generated Flux names a credential and nothing more:\n{flux}"
        );
        assert!(
            !flux.contains(TOKEN_ENV),
            "`{id}` emits `{TOKEN_ENV}`, the variable that resolves the credential. A module names \
             the credential, never the place its value comes from:\n{flux}"
        );
        assert!(
            !flux.contains("Bearer") && !flux.contains("Basic"),
            "`{id}` emits a scheme word Okta does not use:\n{flux}"
        );
    }
}

/// **Deactivating a user is declared as the destructive, non-idempotent write it is.**
///
/// Okta's deactivation ends every one of that person's active sessions, revokes their tokens and
/// grants, and removes their access to every application the org assigns through Okta. It is the
/// operation in this connector that can lock a real human out of their working day, and
/// `risk = "destructive"` — "deletes or otherwise irreversible" — is the only tier that makes flux's
/// approval gate stop for it. The rest of the connector is bounded reads, which this test also pins:
/// a sixth operation quietly added at `medium` would fail here.
#[test]
fn the_user_deactivation_is_the_one_destructive_write_and_the_verify_is_a_read() {
    let connector = load();
    let deactivate = connector
        .operations
        .iter()
        .find(|operation| operation.id == DEACTIVATE_OPERATION)
        .unwrap_or_else(|| panic!("`{DEACTIVATE_OPERATION}` is one of the curated operations"));

    assert_eq!(deactivate.method, HttpMethod::Post);
    assert_eq!(
        deactivate.risk,
        Risk::Destructive,
        "deactivation ends the person's sessions and revokes their app access; `high` would let it \
         through flux's approval gate as an ordinary reviewable write"
    );
    assert_eq!(
        deactivate.idempotency,
        Idempotency::NonIdempotent,
        "a second deactivation is not the same call twice — Okta answers the lifecycle transition \
         differently once the user has left ACTIVE, and this connector does not promise otherwise"
    );

    for operation in &connector.operations {
        if operation.id == DEACTIVATE_OPERATION {
            continue;
        }
        assert_eq!(
            operation.risk,
            Risk::Low,
            "operation `{}` is one of the curated reads and should carry no risk above `low`",
            operation.id
        );
        assert_eq!(operation.method, HttpMethod::Get);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
    }

    // AGENTS.md's configuration contract: a `verify` operation is a read, because it is the "Test
    // connection" button and runs unattended whenever someone opens a settings page.
    let verify = connector
        .operations
        .iter()
        .find(|operation| Some(operation.id.as_str()) == connector.verify.as_deref())
        .expect("the declared `verify` names a declared operation");
    assert_eq!(verify.method, HttpMethod::Get);
    assert_eq!(verify.risk, Risk::Low);
    assert!(
        verify.params.path.is_empty()
            && verify.params.query.iter().all(|param| !param.required)
            && verify.params.body.is_empty(),
        "`{VERIFY_OPERATION}` takes a required argument, so a settings page could not call it \
         unattended"
    );
}

/// **The two shapes this connector cannot express honestly are absent, not guessed at.**
///
/// Okta's list endpoints accept `q`, `filter` and `search` — free text and a SCIM filter expression —
/// and page with an opaque `after` cursor. Neither can ship here today:
///
/// - the filters are the unencodable free-text shape C-30 records against `zendesk-ticket-search`.
///   Query values are interpolated verbatim, so an `&`, `#` or `+` inside an expression corrupts the
///   request, and a `filter` value is *made of* punctuation (`profile.firstName eq "Ada"`);
/// - `after` is only ever handed to a caller in a `Link` **response header**, and this model surfaces
///   a response body. A cursor parameter with no way to learn a cursor is a knob nobody can turn.
///
/// Asserted rather than left in a comment, because the failure mode is a later story adding one back
/// without re-reading why it was left out.
#[test]
fn no_curated_operation_offers_a_free_text_filter_or_a_link_header_cursor() {
    let connector = load();
    for operation in &connector.operations {
        for param in &operation.params.query {
            assert!(
                !EXCLUDED_QUERY_PARAMS.contains(&param.name.as_str()),
                "operation `{}` offers `{}`, which C-161 excluded deliberately — see \
                 `providers/okta.toml`'s header comment and this test's docs before adding it back",
                operation.id,
                param.name
            );
        }
    }
}
