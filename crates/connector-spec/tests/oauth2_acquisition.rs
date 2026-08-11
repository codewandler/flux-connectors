//! **An OAuth2 credential is one acquisition, and never two** — C-525's loader half.
//!
//! The IR has modelled `OAuth2Spec` since C-90 and nothing had ever loaded one, so the first test
//! here is simply that a connector declaring `[auth.oauth2]` is accepted and round-trips every
//! field. That is not ceremony: an unexercised surface is where a `deny_unknown_fields` typo or a
//! missing `#[serde(default)]` hides, and until C-525 no provider and no test declared the block.
//!
//! The refusal is the substance. `AGENTS.md` § Authentication contract already settles that two
//! declarations of one fact with opposite consequences must never sit on one operation, and
//! `validate_one_credential_disposition` enforces that for `credential_response` versus
//! `produces_credential`. This is the same shape one axis over: a credential declaring
//! `[auth.oauth2]` says *the host runs a grant to obtain this*, and an operation's
//! `produces_credential` naming that same credential says *this connector's own call mints it*.
//! Both cannot be how the value arrives. Silently preferring one — which is what an emitter must
//! otherwise do — publishes an acquisition the author did not choose.

use connector_spec::{Connector, OAuthGrant};

/// A minimal well-formed provider with `body` spliced in after the connector-level keys.
fn provider(body: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
base_url = "https://api.acme.example"
description = "A provider that exists to be checked."
{body}
"#
    )
}

/// The bearer credential every case below builds on, with `extra` spliced into its block.
fn credential(extra: &str) -> String {
    format!(
        r#"
[[auth]]
name = "acme.access_token"
scheme = "bearer"
env = ["ACME_ACCESS_TOKEN"]
{extra}
"#
    )
}

/// One ordinary read, so the connector describes something. A provider declaring no operations at
/// all is refused before any credential rule is reached.
const READ: &str = r#"
[[operations]]
id = "acme-thing-list"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List the things."
risk = "low"
idempotency = "idempotent"
"#;

/// The complete OAuth2 block, as a provider would author it.
const OAUTH2: &str = r#"
[auth.oauth2]
endpoint = "login"
authorize_path = "/oauth/authorize"
token_path = "/oauth/token"
client_id = "acme-client"
scopes = ["read:thing", "write:thing"]
grants = ["authorization_code", "refresh_token"]

[auth.oauth2.redirect]
port = 8976
path = "/callback"
"#;

fn load(source: &str) -> connector_spec::Result<Connector> {
    connector_spec::provider::load("providers/acme.toml", source).map(|loaded| loaded.connector)
}

/// The rendered refusal, or a panic naming the provider that was wrongly accepted.
fn refusal(source: &str) -> String {
    match load(source) {
        Ok(_) => panic!("the loader accepted a provider it must refuse:\n{source}"),
        Err(error) => error.to_string(),
    }
}

/// **The declaration loads, and every field survives.**
///
/// Asserted field by field rather than by a spot check: a field silently defaulted here is a grant
/// the host runs against the wrong endpoint, or a scope it never asks for, and both look exactly
/// like a connector that declared them correctly.
#[test]
fn an_oauth2_credential_loads_with_every_field_intact() {
    let source = provider(&format!("{}{READ}", credential(OAUTH2)));
    let connector = load(&source).expect("an oauth2 credential must load");

    let spec = connector.auth[0]
        .oauth2
        .as_ref()
        .expect("the oauth2 block must survive loading");
    assert_eq!(spec.endpoint, "login");
    assert_eq!(spec.authorize_path, "/oauth/authorize");
    assert_eq!(spec.token_path, "/oauth/token");
    assert_eq!(spec.client_id, "acme-client");
    assert_eq!(spec.scopes, ["read:thing", "write:thing"]);
    assert_eq!(
        spec.grants,
        [OAuthGrant::AuthorizationCode, OAuthGrant::RefreshToken]
    );
    let redirect = spec.redirect.as_ref().expect("the redirect must survive");
    assert_eq!(redirect.port, 8976);
    assert_eq!(redirect.path, "/callback");
}

/// A credential with no `[auth.oauth2]` block stays a plain env-to-secret credential.
///
/// The whole catalogue depends on this: `OAuth2Spec` is all-defaulted, so a bug that materialised
/// an empty block would give every one of the 55 shipped connectors an acquisition it never
/// declared.
#[test]
fn a_plain_credential_declares_no_oauth2() {
    let source = provider(&format!("{}{READ}", credential("")));
    let connector = load(&source).expect("a plain credential must load");
    assert!(
        connector.auth[0].oauth2.is_none(),
        "a credential with no `[auth.oauth2]` block gained one"
    );
}

/// **`[auth.oauth2]` and `produces_credential` on one credential is refused**, naming both.
///
/// The refusal has to carry what neither field's own documentation can: which of the two the author
/// meant. A token exchange the *connector* declares as an operation is `produces_credential`; a
/// grant the *host* runs against the vendor's own OAuth endpoints is `[auth.oauth2]`, and an
/// authorize/token endpoint is never a connector operation in the first place.
#[test]
fn declaring_both_an_oauth2_grant_and_a_minting_operation_is_refused() {
    let source = provider(&format!(
        r#"
{}
[[operations]]
id = "acme-token-create"
method = "POST"
direction = "write"
path = "/oauth/token"
description = "Exchange client credentials for an access token."
risk = "medium"
idempotency = "non_idempotent"

[operations.produces_credential]
secret = "/access_token"
credential = "acme.access_token"
"#,
        credential(OAUTH2)
    ));

    let refusal = refusal(&source);
    assert!(
        refusal.contains("acme.access_token"),
        "the refusal must name the credential: {refusal}"
    );
    assert!(
        refusal.contains("acme-token-create"),
        "the refusal must name the minting operation: {refusal}"
    );
    assert!(
        refusal.contains("oauth2") && refusal.contains("produces_credential"),
        "the refusal must name both declarations: {refusal}"
    );
}
