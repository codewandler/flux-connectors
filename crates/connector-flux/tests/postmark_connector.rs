//! Postmark (C-180) is the epic's clean probe for **two credentials partitioned by service rather
//! than sent together**: `X-Postmark-Server-Token` authenticates sending and message history;
//! `X-Postmark-Account-Token` authenticates account-wide server administration. Unlike babelforce's
//! `access_id`/`access_token` pair (`providers/babelforce.toml`), which travel *together* as one
//! mechanism, Postmark's two tokens are never accepted on the same request at all.
//!
//! This file measures the question `providers/postmark.toml`'s header comment poses: is a
//! per-service credential already addressable, and if so, by what mechanism?
//!
//! 1. **The connector declares exactly two credentials**, each on its own custom header, each
//!    resolving from its own environment variable — no scheme in common beyond both being
//!    `AuthScheme::Header`.
//! 2. **Every `server`-service operation resolves, through [`Connector::effective_auth`], to the
//!    server token and only the server token.** Every `account`-service operation resolves to the
//!    account token and only the account token. No operation's effective requirement ever names
//!    both — that is what "never sent together" means at the IR level, since flux has not yet grown
//!    the `$auth` seam that would put a header on the wire at all
//!    (`AGENTS.md`'s Intentional gaps: "No provider can make a live call"). `server` is a *named*
//!    service, not the elided `default`: this provider declares two named services, and
//!    `AGENTS.md`'s service contract refuses an implicit default the moment any named service exists.
//! 3. **The partition is enforced by a mechanism that already shipped**: per-operation `auth`
//!    overriding `default_auth` (`crates/connector-spec/src/ir.rs:652-669`), the same override
//!    babelforce's `[[patch.operations]] auth = [...]` overlay entry already uses on a different
//!    axis. No change to `connector-spec` was needed.
//! 4. **The measured, separate finding**: `Connector::credential_ref_for`
//!    (`crates/connector-spec/src/ir.rs:1166-1178`) still renders both credentials' tenant paths
//!    under the reserved default service — it does not thread an operation's or credential's
//!    `service` through at all. The two paths are nonetheless distinct, because they differ in leaf
//!    name (`server_token` vs `account_token`), exactly as that function's own doc comment predicts
//!    ("credential names are unique within a provider and already distinguish the case",
//!    `ir.rs:1159-1160`). A [`CredentialRef`] *can* carry an arbitrary service segment
//!    (`credential.rs`'s `CredentialRef::new`) — that headroom is real — but nothing in this
//!    connector's path exercises it, and this test measures that rather than asserting it in prose.

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

/// The account-administration credential — never valid for sending, and never sent alongside
/// [`SERVER_TOKEN`] on the same request.
const ACCOUNT_TOKEN: &str = "postmark.account_token";
/// The header `ACCOUNT_TOKEN` travels in.
const ACCOUNT_HEADER: &str = "X-Postmark-Account-Token";
/// A variable *name*; no credential value appears in this repository.
const ACCOUNT_TOKEN_ENV: &str = "POSTMARK_ACCOUNT_TOKEN";

/// The service every sending/message-history operation belongs to. A *named* service, not the
/// elided `default` — see the module docs.
const SERVER_SERVICE: &str = "server";
/// The service every account-administration operation belongs to.
const ACCOUNT_SERVICE: &str = "account";

const BASE_URL: &str = "https://api.postmarkapp.com";

/// The four Server Token operations, in the order `providers/postmark.toml` declares them.
const SERVER_OPERATIONS: &[&str] = &[
    "postmark-email-send",
    "postmark-deliverystats-get",
    "postmark-bounce-list",
    "postmark-bounce-get",
];

/// The two Account Token operations.
const ACCOUNT_OPERATIONS: &[&str] = &["postmark-server-list", "postmark-server-get"];

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

/// **Finding 1: two credentials, two headers, two environment variables — no more shared between
/// them than the fact both are custom headers.**
#[test]
fn the_postmark_connector_declares_two_independent_header_credentials() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Postmark");
    assert_eq!(connector.base_url, BASE_URL);

    assert_eq!(
        connector.auth.len(),
        2,
        "Postmark authenticates sending and account administration with two independent \
         credentials, never a shared one"
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

    let account = connector
        .auth_method(ACCOUNT_TOKEN)
        .unwrap_or_else(|| panic!("postmark declares `{ACCOUNT_TOKEN}`"));
    assert_eq!(
        account.scheme,
        AuthScheme::Header {
            name: ACCOUNT_HEADER.to_string(),
            prefix: String::new(),
        }
    );
    assert_eq!(account.env, [ACCOUNT_TOKEN_ENV]);

    assert_ne!(
        server.env, account.env,
        "the two credentials must never resolve from the same variable"
    );

    assert_eq!(
        connector.default_auth,
        vec![connector_spec::AuthRequirement::single(SERVER_TOKEN)],
        "sending is the connector default; account administration must override it explicitly"
    );
}

/// **Finding 2 and 3: the two tokens are never sent together, and the partition already runs on a
/// mechanism this repository shipped before Postmark — per-operation `auth` overriding
/// `default_auth`.**
///
/// Every operation's *effective* auth (`Connector::effective_auth`, which resolves the
/// inherit-or-override rule) names exactly one credential, and that credential agrees with the
/// operation's own service: `default` operations resolve to the server token, `account` operations
/// resolve to the account token. No [`connector_spec::AuthRequirement`] anywhere in this connector
/// ever names both tokens in the same alternative.
#[test]
fn no_operation_ever_requires_both_tokens_and_each_resolves_to_its_own_services_token() {
    let connector = load();

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    let mut expected: Vec<&str> = SERVER_OPERATIONS.to_vec();
    expected.extend_from_slice(ACCOUNT_OPERATIONS);
    assert_eq!(declared, expected);

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "`{}` must have exactly one auth alternative — Postmark offers no OR choice between \
             the two tokens",
            operation.id
        );
        let requirement = &effective[0];

        assert_eq!(
            requirement.len(),
            1,
            "`{}`'s one alternative must name exactly one credential — the whole point of this \
             connector is that the two tokens are never an AND-group on the same request",
            operation.id
        );

        let expected_credential = if operation.service == ACCOUNT_SERVICE {
            ACCOUNT_TOKEN
        } else {
            assert_eq!(
                operation.service, SERVER_SERVICE,
                "`{}` belongs to a service this test does not expect",
                operation.id
            );
            SERVER_TOKEN
        };
        assert!(
            requirement.contains(expected_credential),
            "`{}` (service {:?}) must resolve to `{expected_credential}`, not to the other \
             service's token",
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

    let account_ids: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| operation.service == ACCOUNT_SERVICE)
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(account_ids, ACCOUNT_OPERATIONS);
}

/// **Finding 4, measured rather than asserted: `credential_ref_for` does not thread a per-credential
/// service through, even though [`CredentialRef`] can carry one.**
///
/// Both credentials render under the elided default service — telling them apart by leaf name
/// alone, exactly as `Connector::credential_ref_for`'s own doc comment (`ir.rs:1159-1160`) predicts.
/// This is the fact that makes the connector correct *without* that headroom, not a defect this
/// story fixes: nothing here needs `credential_ref_for` to change, because nothing today asks two
/// *same-named* credentials in different services to coexist.
#[test]
fn credential_ref_for_elides_the_service_and_the_two_tokens_still_never_collide() {
    let connector = load();

    let server_ref = connector
        .credential_ref_for("9f3a4b2c", SERVER_TOKEN, TenantInstances::sole())
        .expect("a valid tenant id and a declared credential must resolve")
        .expect("postmark declares an authority, so a reference must render");
    let account_ref = connector
        .credential_ref_for("9f3a4b2c", ACCOUNT_TOKEN, TenantInstances::sole())
        .expect("a valid tenant id and a declared credential must resolve")
        .expect("postmark declares an authority, so a reference must render");

    assert!(
        server_ref.is_default_service() && account_ref.is_default_service(),
        "credential_ref_for renders every credential under the elided default service today — \
         both tokens, not just one — which is the measured finding this test exists to pin"
    );
    assert_ne!(
        server_ref, account_ref,
        "the two credentials must still be distinct addresses even though neither carries a \
         service segment — they differ by leaf name, `server_token` vs `account_token`"
    );
    assert_eq!(server_ref.credential(), "server_token");
    assert_eq!(account_ref.credential(), "account_token");

    let rendered_server = TenantLayout.render(&server_ref);
    let rendered_account = TenantLayout.render(&account_ref);
    assert_eq!(
        rendered_server,
        "tenants/9f3a4b2c/com.postmarkapp.api/server_token"
    );
    assert_eq!(
        rendered_account,
        "tenants/9f3a4b2c/com.postmarkapp.api/account_token"
    );
    // Four segments each (`tenants/<tenant>/<authority>/<credential>`), not the five a
    // service-scoped path would carry (`tenants/<tenant>/<authority>/<service>/<credential>`) — the
    // headroom on `CredentialRef::new` exists, but this connector's path through
    // `credential_ref_for` does not reach it, for either credential.
    assert_eq!(rendered_server.split('/').count(), 4);
    assert_eq!(rendered_account.split('/').count(), 4);

    // The headroom itself is real, even though nothing on this connector's resolution path uses
    // it: a host is free to construct a service-scoped reference directly.
    let hypothetical = CredentialRef::new(
        "9f3a4b2c",
        "com.postmarkapp.api",
        ACCOUNT_SERVICE,
        "account_token",
    )
    .expect("CredentialRef::new accepts an arbitrary service — the headroom is real");
    assert_ne!(
        hypothetical, account_ref,
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
