//! **An OAuth2 declaration may place its token endpoint on a second declared service** — C-556.
//!
//! `OAuth2Spec` binds `authorize_path` and `token_path` to one declared service through its
//! `endpoint` name. Anthropic's subscription flow authorizes on `claude.ai` and redeems its token on
//! `platform.claude.com`, which one endpoint cannot express. The fix is an optional second service
//! *reference* — [`OAuth2Spec::token_endpoint`], a declared service **name** and never a URL, so the
//! host set stays derived from declared services and `http_hosts`, declared-authority validation and
//! X-154's `NoDeclaredDefault` composition rule all keep working. Absent means today's behaviour,
//! byte-for-byte.
//!
//! C-556 also lands the public-vs-confidential discriminator the same flow needs
//! ([`OAuth2Spec::public_client`]): a PKCE public client issues and uses no client secret, so it must
//! not be required to publish one. This file proves both additions at the loader; the archetype form
//! matrix that reads the discriminator lives in `auth_archetypes.rs`.
//!
//! These tests use a **synthetic** connector, never a shipped provider — a shipped provider's TOML is
//! another story's write set, and the point here is the loader rule, which a fixture exercises
//! exactly.

use connector_spec::{provider, Connector};

/// A two-service fixture: the authorize host and the token host are different declared services, plus
/// the API surface the one operation belongs to. `oauth` is spliced in so a variant can move the
/// `token_endpoint` (or drop it) without restating the frame.
fn two_host_provider(oauth: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"
description = "A fixture connector for the two-host OAuth2 token endpoint."

[[services]]
name = "api"
description = "The API surface every operation reaches."

[[services]]
name = "authz"
description = "The OAuth2 authorize host."
base_url = "https://auth.acme.example"

[[services]]
name = "tokensvc"
description = "The OAuth2 token host, distinct from the authorize host."
base_url = "https://token.acme.example"

[[auth]]
name = "acme.oauth"
scheme = "bearer"
env = ["ACME_ACCESS_TOKEN"]
subject = "user"

[auth.oauth2]
{oauth}

[[operations]]
service = "api"
id = "acme-ping"
method = "GET"
direction = "read"
path = "/ping"
risk = "low"
idempotency = "idempotent"
"#
    )
}

fn load(source: &str) -> connector_spec::Result<Connector> {
    provider::load("providers/acme.toml", source).map(|loaded| loaded.connector)
}

/// The loaded credential's `oauth2` block, re-serialized to JSON. Reading it back through serde
/// rather than through the struct keeps this test honest about the *wire* shape: an added field that
/// is skipped when absent, and present when declared, is exactly what the serialization must show.
fn oauth2_json(connector: &Connector) -> serde_json::Value {
    let method = &connector.auth[0];
    let value = serde_json::to_value(method).expect("an auth method serializes");
    value["oauth2"].clone()
}

/// **A two-host declaration loads and carries both services.** The authorize service and the token
/// service differ, and both survive lowering into the credential's `oauth2` block.
#[test]
fn a_two_host_declaration_loads_and_carries_both_services() {
    let source = two_host_provider(
        r#"endpoint = "authz"
token_endpoint = "tokensvc"
authorize_path = "/oauth/authorize"
token_path = "/oauth/token"
client_id = ""
grants = ["authorization_code"]"#,
    );
    let connector = load(&source).expect("a two-host OAuth2 declaration must load");

    let oauth2 = oauth2_json(&connector);
    assert_eq!(
        oauth2["endpoint"], "authz",
        "the authorize service must survive: {oauth2}"
    );
    assert_eq!(
        oauth2["token_endpoint"], "tokensvc",
        "the token service must survive as a distinct declared name: {oauth2}"
    );
}

/// **Absent `token_endpoint` is byte-for-byte today's behaviour.** A single-host declaration
/// serializes with no `token_endpoint` key at all — the property that keeps every committed document
/// from moving.
#[test]
fn an_absent_token_endpoint_is_skipped_entirely() {
    let source = two_host_provider(
        r#"endpoint = "authz"
authorize_path = "/oauth/authorize"
token_path = "/oauth/token"
client_id = ""
grants = ["authorization_code"]"#,
    );
    let connector = load(&source).expect("a single-host OAuth2 declaration must load");

    let oauth2 = oauth2_json(&connector);
    assert!(
        oauth2.get("token_endpoint").is_none(),
        "an undeclared token_endpoint must not appear in the wire form: {oauth2}"
    );
}

/// **A `token_endpoint` that names no declared service is refused, naming it.** The name is a
/// service reference, validated like `endpoint`: a typo points the token exchange at a host the
/// allow-list never admitted, so it is a loud loader refusal rather than a silent dangling name.
#[test]
fn a_dangling_token_endpoint_is_refused_naming_it() {
    let source = two_host_provider(
        r#"endpoint = "authz"
token_endpoint = "ghost"
authorize_path = "/oauth/authorize"
token_path = "/oauth/token"
client_id = ""
grants = ["authorization_code"]"#,
    );
    let error = load(&source)
        .expect_err("a dangling token_endpoint must be refused")
        .to_string();

    assert!(
        error.contains("ghost"),
        "the refusal must name the undeclared service: {error}"
    );
    assert!(
        error.contains("is not a declared service"),
        "the refusal must say the token_endpoint names no declared service, not merely reject an \
         unknown key: {error}"
    );
}

/// **A public PKCE client loads and is marked public.** The discriminator is additive: a confidential
/// client omits it and is unchanged; a public client carries `public_client = true` into the wire
/// form for a consumer to read.
#[test]
fn a_public_client_loads_and_is_marked_public() {
    let source = two_host_provider(
        r#"endpoint = "authz"
authorize_path = "/oauth/authorize"
token_path = "/oauth/token"
client_id = ""
public_client = true
grants = ["authorization_code"]"#,
    );
    let connector = load(&source).expect("a public-client OAuth2 declaration must load");

    let oauth2 = oauth2_json(&connector);
    assert_eq!(
        oauth2["public_client"], true,
        "a public client must publish the discriminator: {oauth2}"
    );
}

/// **A confidential client omits the discriminator entirely.** `public_client = false` is the default
/// and is skipped when serializing, so every already-shipped confidential declaration is unaffected.
#[test]
fn a_confidential_client_omits_the_discriminator() {
    let source = two_host_provider(
        r#"endpoint = "authz"
authorize_path = "/oauth/authorize"
token_path = "/oauth/token"
client_id = ""
grants = ["authorization_code"]"#,
    );
    let connector = load(&source).expect("a confidential OAuth2 declaration must load");

    let oauth2 = oauth2_json(&connector);
    assert!(
        oauth2.get("public_client").is_none(),
        "an unset public_client must not appear in the wire form: {oauth2}"
    );
}
