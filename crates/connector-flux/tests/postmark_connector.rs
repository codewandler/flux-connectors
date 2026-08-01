//! Postmark (C-180) was this epic's clean probe for **two credentials partitioned by service rather
//! than sent together**: `X-Postmark-Server-Token` authenticates sending and message history;
//! `X-Postmark-Account-Token` authenticated account-wide server administration, and unlike
//! babelforce's `access_id`/`access_token` pair (`providers/babelforce.toml`), which travel
//! *together* as one mechanism, Postmark's two tokens were never accepted on the same request at
//! all.
//!
//! **C-430 withheld the Account API surface and the probe went with it.** `postmark-server-list` and
//! `postmark-server-get` returned `ApiTokens` — that server's own live Server Token(s) in plaintext
//! — and an operation whose declared response carries a token is withheld until C-136's diversion
//! lands (`AGENTS.md` § Authentication contract). They were the only two operations in the `account`
//! service, a service with no operations is refused, and a credential nothing can use is not asked
//! for: so the service, `postmark.account_token` and its config field went too. What this file
//! measured on a live two-service connector is therefore recorded below as a **finding, not a
//! measurement**, and the tests that remain are the ones a single-credential connector can still
//! prove. C-136 is what restores both the operations and the probe.
//!
//! The finding, as it was measured before the withholding, in the terms
//! `providers/postmark.toml`'s header comment poses it:
//!
//! 1. **The connector declared exactly two credentials**, each on its own custom header, each
//!    resolving from its own environment variable — no scheme in common beyond both being
//!    `AuthScheme::Header`. One survives, and [`the_postmark_connector_declares_one_header_credential`]
//!    still measures its half.
//! 2. **Every `server`-service operation resolves, through [`Connector::effective_auth`], to the
//!    server token and only the server token.** Every `account`-service operation resolved to the
//!    account token and only the account token; no operation's effective requirement ever named
//!    both — that is what "never sent together" meant at the IR level. The surviving half is still
//!    asserted, and `server` is still a *named* service rather than the elided `default`.
//! 3. **The partition was enforced by a mechanism that already shipped**: per-operation `auth`
//!    overriding `default_auth`, the same override babelforce's `[[patch.operations]] auth = [...]`
//!    overlay entry uses on a different axis. No change to `connector-spec` was needed for it, and
//!    none is needed to undo it — the withheld operations carried their own `auth` and took it with
//!    them.
//! 4. **The measured, separate finding, and the one that survives intact**:
//!    `Connector::credential_ref_for` renders a credential's tenant path under the reserved default
//!    service — it does not thread an operation's or credential's `service` through at all. A
//!    [`CredentialRef`] *can* carry an arbitrary service segment (`credential.rs`'s
//!    `CredentialRef::new`) — that headroom is real — but nothing in this connector's path exercises
//!    it, and this file measures that rather than asserting it in prose.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, CredentialRef, Layout, TenantInstances, TenantLayout};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider under test.
const PROVIDER: &str = "postmark";

/// The sending credential — the connector default, and the only one every `server`-service operation
/// may resolve to.
const SERVER_TOKEN: &str = "postmark.server_token";
/// The header `SERVER_TOKEN` travels in.
const SERVER_HEADER: &str = "X-Postmark-Server-Token";
/// A variable *name*; no credential value appears in this repository.
const SERVER_TOKEN_ENV: &str = "POSTMARK_SERVER_TOKEN";

/// The service every sending/message-history operation belongs to. A *named* service, not the
/// elided `default` — see the module docs. It is the only service left; `account` was withheld with
/// the two operations that were all it carried.
const SERVER_SERVICE: &str = "server";

/// A service name this connector no longer declares, kept because
/// [`credential_ref_for_elides_the_service`] uses it to build the *hypothetical* service-scoped
/// reference that shows the headroom on [`CredentialRef::new`] is real — which is a claim about the
/// address type, not about Postmark, and so outlives the surface that motivated it.
const HYPOTHETICAL_SERVICE: &str = "account";

const BASE_URL: &str = "https://api.postmarkapp.com";

/// The four Server Token operations, in the order `providers/postmark.toml` declares them — and now
/// the whole connector. `postmark-server-list` and `postmark-server-get` stood beside them under the
/// `account` service until C-430 withheld both; `crates/connector-spec/tests/credential_response.rs`
/// is where that exclusion is recorded and checked.
const SERVER_OPERATIONS: &[&str] = &[
    "postmark-email-send",
    "postmark-deliverystats-get",
    "postmark-bounce-list",
    "postmark-bounce-get",
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
            "cannot read {} ({error}) — C-180 ships the Postmark connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// **Finding 1, in the half that survives: one credential on its own custom header, resolving from
/// its own environment variable.**
///
/// The Account Token stood beside it until C-430 and is asserted *absent* here rather than merely
/// unmentioned — a credential a withheld surface left behind would be a connector asking a human for
/// a token nothing it declares can use, which the configuration contract forbids and which no other
/// test in this repository would catch.
#[test]
fn the_postmark_connector_declares_one_header_credential() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Postmark");
    assert_eq!(connector.base_url, BASE_URL);

    assert_eq!(
        connector.auth.len(),
        1,
        "Postmark's account-administration credential went with the Account API surface C-430 \
         withheld; nothing this connector declares can use one"
    );

    let server = connector
        .auth_method(SERVER_TOKEN)
        .unwrap_or_else(|| panic!("postmark declares `{SERVER_TOKEN}`"));
    assert_eq!(
        server.scheme,
        AuthScheme::Header {
            name: SERVER_HEADER.to_string(),
            prefix: String::new(),
        }
    );
    assert_eq!(server.env, [SERVER_TOKEN_ENV]);

    assert!(
        connector.auth_method("postmark.account_token").is_none(),
        "the Account Token is withheld with the operations that used it — a credential nothing can \
         use is a form the operator fills in for nothing"
    );
    assert!(
        connector
            .config
            .iter()
            .all(|field| field.name != "account_token"),
        "the config field asking a human for the Account Token must go with the credential"
    );

    assert_eq!(
        connector.default_auth,
        vec![connector_spec::AuthRequirement::single(SERVER_TOKEN)],
        "sending is the connector default, and now the only thing to default to"
    );
}

/// **Finding 2 and 3, in the half that survives: every operation resolves through
/// [`Connector::effective_auth`] to the server token and only the server token.**
///
/// The partition ran on a mechanism this repository shipped before Postmark — per-operation `auth`
/// overriding `default_auth` — and the withheld operations carried their own override and took it
/// with them, so what remains is a connector where every operation inherits. That is a weaker claim
/// than the one this test used to make, and it is stated as the weaker claim rather than dressed up:
/// the "never sent together" property is no longer demonstrable here, because there is no longer a
/// second token to send.
#[test]
fn every_operation_resolves_to_the_one_service_token() {
    let connector = load();

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, SERVER_OPERATIONS);

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "`{}` must have exactly one auth alternative — Postmark offers no OR choice of token",
            operation.id
        );
        let requirement = &effective[0];

        assert_eq!(
            requirement.len(),
            1,
            "`{}`'s one alternative must name exactly one credential; two would be an AND-group on \
             one request, which this vendor never accepts",
            operation.id
        );

        assert_eq!(
            operation.service, SERVER_SERVICE,
            "`{}` belongs to a service this connector no longer declares",
            operation.id
        );
        assert!(
            requirement.contains(SERVER_TOKEN),
            "`{}` (service {:?}) must resolve to `{SERVER_TOKEN}`",
            operation.id,
            operation.service
        );
    }

    let server_ids: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| operation.service == SERVER_SERVICE)
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(server_ids, SERVER_OPERATIONS);
}

/// **Finding 4, measured rather than asserted: `credential_ref_for` does not thread a per-credential
/// service through, even though [`CredentialRef`] can carry one.**
///
/// The credential renders under the elided default service — identified by leaf name alone, exactly
/// as `Connector::credential_ref_for`'s own doc comment predicts. This is the fact that made the
/// connector correct *without* that headroom, not a defect any story fixes: nothing here needs
/// `credential_ref_for` to change, because nothing today asks two *same-named* credentials in
/// different services to coexist.
///
/// It measured **both** tokens eliding the service until C-430 withheld the surface the second one
/// authenticated. The claim is unchanged and its evidence is now one credential wide, which is worth
/// saying rather than quietly narrowing the assertion: `server` is a **named** service and its own
/// credential still renders no service segment, so the elision is a property of `credential_ref_for`
/// rather than of a connector that happened to have only a default service.
#[test]
fn credential_ref_for_elides_the_service() {
    let connector = load();

    let server_ref = connector
        .credential_ref_for("9f3a4b2c", SERVER_TOKEN, TenantInstances::sole())
        .expect("a valid tenant id and a declared credential must resolve")
        .expect("postmark declares an authority, so a reference must render");

    assert!(
        server_ref.is_default_service(),
        "credential_ref_for renders every credential under the elided default service today, even \
         one whose operations belong to a *named* service — the measured finding this test pins"
    );
    assert_eq!(server_ref.credential(), "server_token");

    let rendered_server = TenantLayout.render(&server_ref);
    assert_eq!(
        rendered_server,
        "tenants/9f3a4b2c/com.postmarkapp.api/server_token"
    );
    // Four segments (`tenants/<tenant>/<authority>/<credential>`), not the five a service-scoped
    // path would carry (`tenants/<tenant>/<authority>/<service>/<credential>`) — the headroom on
    // `CredentialRef::new` exists, but this connector's path through `credential_ref_for` does not
    // reach it.
    assert_eq!(rendered_server.split('/').count(), 4);

    // The headroom itself is real, even though nothing on this connector's resolution path uses
    // it: a host is free to construct a service-scoped reference directly.
    let hypothetical = CredentialRef::new(
        "9f3a4b2c",
        "com.postmarkapp.api",
        HYPOTHETICAL_SERVICE,
        "server_token",
    )
    .expect("CredentialRef::new accepts an arbitrary service — the headroom is real");
    assert_ne!(
        hypothetical, server_ref,
        "a hand-built service-scoped reference differs from what credential_ref_for actually \
         produces today, which is the gap this test measures rather than closes"
    );
}

/// Every curated operation actually emits: this connector's flat send-mail body (`From`, `To`,
/// `Subject`, `HtmlBody`, `TextBody` as body-root scalars) is exactly what SendGrid's own send
/// operation could not express (`providers/sendgrid.toml`'s header comment) — no array nesting, so
/// nothing here exercises `BodyNode`'s missing array variant.
#[test]
fn every_curated_operation_emits() {
    let connector = load();
    for operation in &connector.operations {
        emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
    }
}

/// No recipient or sender address, real or plausible-looking, appears anywhere in the provider
/// definition — not as a config `example`, not as a JSON Schema `example`/`default`, not as a
/// literal in a description. This greps the raw TOML source rather than the parsed IR so a stray
/// literal cannot hide behind a field this test does not otherwise inspect.
#[test]
fn no_email_address_literal_appears_anywhere_in_the_provider_file() {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path).expect("providers/postmark.toml exists");

    // A minimal, deliberately loose address-shaped pattern: `word@word.word`. Anything matching
    // this in a hand-authored provider file is either a real address (never allowed) or a
    // plausible-looking placeholder (the failure mode `providers/sendgrid.toml` and
    // `providers/calendly.toml` already guard against) — so any match at all is the failure.
    for (line_number, line) in source.lines().enumerate() {
        let bytes = line.as_bytes();
        for (index, &byte) in bytes.iter().enumerate() {
            if byte != b'@' {
                continue;
            }
            let before = &line[..index];
            let after = &line[index + 1..];
            let has_local = before
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric());
            let has_domain_dot = after.contains('.')
                && after
                    .chars()
                    .take_while(|c| *c != ' ' && *c != '"')
                    .any(|c| c.is_ascii_alphabetic());
            assert!(
                !(has_local && has_domain_dot),
                "line {} of providers/postmark.toml looks like it contains an email address: {:?}",
                line_number + 1,
                line
            );
        }
    }
}
