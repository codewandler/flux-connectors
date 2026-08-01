//! **What form does each kind of authentication generate?** — C-22's conformance matrix, asked from
//! the configuration side.
//!
//! C-22 pins the auth model against one case per real-world archetype so that an unsupported
//! credential shape fails loudly in a test rather than silently at the first live request. This is
//! that matrix, with the question sharpened by what a hosted product actually needs: it is not enough
//! that the model can *express* an archetype, it must be able to say **what to ask a human for**.
//!
//! Each case names the real provider it is drawn from, so the matrix documents reality rather than
//! hypotheticals. An archetype the model cannot render is an explicit failing case here, not a gap
//! discovered when someone tries to build the form.

use connector_spec::{provider, AuthScheme, Binding, Connector, Format, Level};

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

fn shipped(name: &str) -> Connector {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../providers")
        .join(format!("{name}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    shipped_provider::load_definition(name, &source)
        .unwrap_or_else(|e| panic!("providers/{name}.toml must load:\n{e}"))
        .connector
}

/// The fields a form would render for a connector, as (label, secret, level) triples in order.
fn form(connector: &Connector) -> Vec<(&str, bool, Level)> {
    connector
        .config
        .iter()
        .map(|field| {
            (
                field.label.as_str(),
                field.secret,
                field.level().expect("a loaded field has a valid binding"),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Archetype 1 — prefixed header (`Authorization: Bearer`). Drawn from slack.
//
// The most common archetype there is, and the one whose form is a single masked input. Note what the
// model does NOT say: nothing distinguishes a bearer a user pastes from a bearer OAuth mints, so the
// form is derivable only because the config field says so explicitly.
// ---------------------------------------------------------------------------------------------

#[test]
fn bearer_paste_a_token() {
    let connector = shipped("slack");
    let method = connector
        .auth_method("slack.bot_token")
        .expect("slack declares a bot token");
    assert_eq!(method.scheme, AuthScheme::Bearer);
    assert_eq!(method.env, ["SLACK_BOT_TOKEN"]);

    // Slack's host is literal, so its form has no tenant field — the whole connection is one secret.
    // It declares no `[[config]]` today, which is legitimate and is exactly what the OAuth story will
    // change; asserted so that the change is visible when it happens.
    assert!(
        connector.config.is_empty(),
        "slack has no tenant and no declared config; when its OAuth alternative lands this must be \
         revisited deliberately rather than drifting"
    );
}

// ---------------------------------------------------------------------------------------------
// Archetype 2 — basic join with a vendor marker. Drawn from zendesk.
//
// The archetype that most needs a generated form, because a human cannot guess it: the username half
// is an email, the password half is a token, and the vendor appends its own `/token` marker which the
// user must NOT type. Three facts, none of which a bare `scheme = "basic"` conveys.
// ---------------------------------------------------------------------------------------------

#[test]
fn basic_join_renders_two_fields_and_hides_the_vendor_marker() {
    let connector = shipped("zendesk");
    let method = connector
        .auth_method("zendesk.api_token")
        .expect("declared");
    assert_eq!(method.scheme, AuthScheme::Basic);
    assert_eq!(method.user_suffix.as_deref(), Some("/token"));

    assert_eq!(
        form(&connector),
        vec![
            ("Zendesk subdomain", false, Level::Connection),
            ("Agent email", false, Level::Connection),
            ("API token", true, Level::Connection),
        ]
    );

    // The `/token` marker is public API syntax the host appends, and the help text says so — a user
    // who typed it would double it.
    let email = connector.config_field("email").expect("declared");
    assert!(
        email.help.contains("you do not type that part"),
        "a form must tell the user not to type the vendor's marker: {:?}",
        email.help
    );
    assert_eq!(email.format, Format::Email);
}

/// Jira is the same archetype **without** the marker, and the pair proves the difference is declared
/// rather than assumed: a form generated for Jira must not tell the user about a `/token` suffix.
#[test]
fn basic_join_without_a_marker_is_a_distinct_form() {
    let connector = shipped("jira");
    let method = connector.auth_method("jira.api_token").expect("declared");
    assert_eq!(method.scheme, AuthScheme::Basic);
    assert_eq!(
        method.user_suffix, None,
        "jira sends the email exactly as typed"
    );

    let email = connector.config_field("email").expect("declared");
    assert!(
        !email.help.contains("/token"),
        "jira's form must not mention zendesk's marker: {:?}",
        email.help
    );
}

// ---------------------------------------------------------------------------------------------
// Archetype 3 — raw-value custom header. Drawn from shopify.
//
// The header's entire value is the secret, with no scheme word. Indistinguishable from a bearer in
// the form it generates, which is the point: placement is the host's business, not the user's.
// ---------------------------------------------------------------------------------------------

#[test]
fn raw_value_header_renders_the_same_form_as_a_bearer() {
    let connector = shipped("shopify");
    let method = connector
        .auth_method("shopify.access_token")
        .expect("declared");
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: "X-Shopify-Access-Token".to_owned(),
            prefix: String::new(),
        }
    );

    assert_eq!(
        form(&connector),
        vec![
            ("Shop name", false, Level::Connection),
            ("Admin API access token", true, Level::Connection),
        ],
        "a user supplies a tenant and a secret; which header carries it is not their concern"
    );
}

// ---------------------------------------------------------------------------------------------
// Archetype 4 — no credential at all. Drawn from freshdesk.
//
// A form with one field and no secret. The connector then fails closed with a 401, which is honest
// and visible — as against a form that collected a key nothing would gate.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_connector_with_no_credential_still_has_a_form() {
    let connector = shipped("freshdesk");
    assert!(
        connector.auth.is_empty(),
        "freshdesk deliberately declares no credential — see its header"
    );
    assert_eq!(
        form(&connector),
        vec![("Freshdesk domain", false, Level::Connection)]
    );
}

// ---------------------------------------------------------------------------------------------
// Archetype 5 — two credentials sent together (AND), and alternatives (OR). Drawn from babelforce.
//
// The OR/AND structure of `AuthRequirement` is the strongest form primitive the model already had
// before any of this: alternatives are tabs, and an AND-set is a group of fields that must be filled
// together.
// ---------------------------------------------------------------------------------------------

#[test]
fn and_sets_and_or_alternatives_are_the_grouping_a_form_renders() {
    let connector = shipped("babelforce");
    let alternatives = &connector.default_auth;

    assert!(
        !alternatives.is_empty(),
        "babelforce declares a default requirement set"
    );
    for mechanism in alternatives {
        for credential in mechanism.iter() {
            assert!(
                connector.auth_method(credential).is_some(),
                "every credential in a mechanism resolves, which is what lets a form group by it"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Archetype 6 — a signing secret. Drawn from slack's Events API binding.
//
// Inbound-only: it verifies bytes that arrived and is never placed in a request. A form must still
// collect it, which is the whole reason it is an `[[auth]]` entry rather than living in a second
// namespace a product would have to know about separately.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_signing_secret_is_collected_like_any_credential_and_sent_nowhere() {
    let connector = shipped("slack");
    let method = connector
        .auth_method("slack.signing_secret")
        .expect("slack declares a signing secret");
    assert_eq!(method.scheme, AuthScheme::Signing);

    // It is referenced by the webhook binding's verification, and by nothing outbound.
    let binding = connector.channel("events-api").expect("declared");
    let Some(connector_spec::VerificationScheme::Hmac(hmac)) = &binding.verification else {
        panic!("the events-api binding verifies with HMAC");
    };
    assert_eq!(hmac.secret, "slack.signing_secret");

    for operation in &connector.operations {
        for mechanism in connector.effective_auth(operation) {
            assert!(
                !mechanism.contains("slack.signing_secret"),
                "operation {:?} must not authenticate with a signing secret",
                operation.id
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Archetype 7 — OAuth2. THE EXPLICIT FAILING CASE.
//
// C-22 requires that an archetype the model cannot render is a documented failing case rather than a
// gap found later. This is it, and it is the reason the epic's operator-level half is unproven.
// ---------------------------------------------------------------------------------------------

/// `OAuth2Spec` is a landed type that **no shipped provider uses**, so the operator level of the
/// configuration model — `oauth.client_id`, `oauth.client_secret` — is currently exercised only by
/// fixtures.
///
/// This test asserts the gap rather than papering over it. When a provider adopts OAuth it will fail,
/// and the fix is to assert the generated form instead: an operator-level client id and secret, and a
/// connection-level "Connect" button rather than a token input.
#[test]
fn no_shipped_provider_exercises_oauth_yet() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../providers");
    let mut with_oauth: Vec<String> = Vec::new();
    let mut checked = 0;

    for entry in std::fs::read_dir(&dir).expect("providers/ is readable") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let name = path
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .to_string();
        let connector = shipped(&name);
        if connector.auth.iter().any(|m| m.oauth2.is_some()) {
            with_oauth.push(name);
        }
        checked += 1;
    }
    assert!(checked > 0, "no providers were checked");

    assert!(
        with_oauth.is_empty(),
        "{with_oauth:?} now declare `[auth.oauth2]`. That is the intended direction — but this test \
         encoded the fact that the operator level of the configuration model was unproven, so \
         replace it with an assertion about the form OAuth generates: an operator-level client id \
         and client secret, and a connection-level consent step rather than a pasted token"
    );
}

/// The operator level is nonetheless *reachable*, proven over a fixture rather than a provider — so
/// the two-level model is not merely asserted in prose.
#[test]
fn the_operator_level_is_expressible() {
    let source = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"

[[auth]]
name = "acme.oauth"
scheme = "bearer"
env = ["ACME_ACCESS_TOKEN"]

[auth.oauth2]
endpoint = "acme"
authorize_path = "/oauth/authorize"
token_path = "/oauth/token"
client_id = ""
grants = ["authorization_code"]

[[operations]]
id = "acme-ping"
method = "GET"
path = "/ping"
risk = "low"
idempotency = "idempotent"

[[config]]
name = "client_id"
label = "Client ID"
help = "From the app you registered with Acme"
binds = "oauth.client_id"

[[config]]
name = "client_secret"
label = "Client secret"
help = "From the same app registration. Shown once"
secret = true
binds = "oauth.client_secret"
"#;
    let connector = provider::load("providers/fixture.toml", source)
        .expect("the fixture must load")
        .connector;

    assert_eq!(
        form(&connector),
        vec![
            ("Client ID", false, Level::Operator),
            ("Client secret", true, Level::Operator),
        ],
        "an app registration is set once per vendor, not once per tenant — asking every end user \
         for a client secret would hand them the product's own credential"
    );

    assert!(matches!(
        connector
            .config_field("client_secret")
            .and_then(|f| f.binding()),
        Some(Binding::OAuthClientSecret)
    ));
}
