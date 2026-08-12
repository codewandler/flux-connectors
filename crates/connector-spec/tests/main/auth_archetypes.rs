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

use crate::shipped_provider;

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

    let support_form: Vec<_> = connector
        .config_of(connector_spec::DEFAULT_SERVICE)
        .map(|field| {
            (
                field.label.as_str(),
                field.secret,
                field.level().expect("a loaded field has a valid binding"),
            )
        })
        .collect();
    assert_eq!(
        support_form,
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
// Archetype 7 — OAuth2. THE CASE THAT WAS FAILING AND NOW SHIPS.
//
// C-22 required that an archetype the model cannot render be a documented failing case rather than a
// gap found later. It was one until C-530, and the test that recorded it named its own successor:
// "replace it with an assertion about the form OAuth generates: an operator-level client id and
// client secret, and a connection-level consent step rather than a pasted token". This is that
// assertion.
// ---------------------------------------------------------------------------------------------

/// **Every OAuth2 connector generates the operator/connection split the model promises.**
///
/// The predecessor asserted that no provider declared `[auth.oauth2]` at all, which made the
/// operator level of the configuration model exercised only by fixtures. What replaces it is the
/// shape rather than the absence, checked over the real corpus:
///
/// - the app registration is **operator** level and its secret is marked secret — asking a tenant
///   for a client secret hands them the product's own credential, which is the defect the two-level
///   model exists to prevent, and `Level` is derived from `binds` so an author cannot state it
///   wrongly. The secret is owed only by a **confidential** client (C-556): a public PKCE client
///   (`public_client = true`) issues none, so it declares its `client_id` and redirect URI but no
///   secret, and the test below reads the discriminator rather than requiring the field of every
///   `authorization_code` connector;
/// - the grant names a **declared service** rather than carrying a URL, which is what keeps the
///   token exchange inside the host allow-list;
/// - the credential states its [`Subject`], because a delegated token that cannot say whose
///   authority it carries is one a host cannot bound.
///
/// # Two of those were written about `authorization_code` and said "every grant" (C-440)
///
/// The assertions were correct and their scope was not, which only became visible when a second
/// OAuth connector landed declaring a different grant. Both narrowings are the *document's* own
/// distinction rather than a convenience:
///
/// - **The registration triple is `authorization_code`'s.** A client id, a client secret and a
///   redirect URI are issued together when somebody registers an application, and a redirect URI has
///   no meaning without a browser leg — babelforce's `OAuthTokenRequest` marks `redirect_uri`
///   "`authorization_code` only" and `client_id` "Required for `authorization_code` and
///   `client_credentials`". A resource-owner password grant needs none of the three: the operator
///   already has the username and password. Requiring them anyway would have forced a connector to
///   publish an operator experience nobody had verified the vendor offers, which is the failure
///   `AGENTS.md` records as "Acceptance must not assert a mechanism nobody has verified exists".
///   `authorize_path` is narrowed with them, for the same reason: it is the browser leg's endpoint.
/// - **An empty `endpoint` is the one host that is certainly admitted.** The assertion's premise is
///   that a grant naming no declared endpoint reaches a host the allow-list never admitted — and
///   that is exactly backwards for the empty value, which `OAuth2Spec::endpoint` documents as *the
///   connector's own base URL*. That host is where every operation already goes, so it is admitted by
///   construction. What must still be refused is a **named** endpoint that names no service, which is
///   a typo pointing the token exchange at nothing.
///
/// What did **not** narrow: [`Subject`], which every grant owes an answer to, and `token_path`, which
/// every grant POSTs to.
#[test]
fn every_oauth_connector_generates_the_operator_connection_split() {
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
        checked += 1;

        let granted: Vec<_> = connector
            .auth
            .iter()
            .filter(|method| method.oauth2.is_some())
            .collect();
        if granted.is_empty() {
            continue;
        }
        with_oauth.push(name.clone());

        let level_of = |binds: &str| {
            connector
                .config
                .iter()
                .find(|field| field.binds == binds)
                .map(|field| (field.level(), field.secret))
        };

        // The registration triple belongs to the browser leg — see this test's docs. A connector
        // that runs only a password or refresh grant asks an operator for nothing to register.
        let redirects = granted.iter().any(|method| {
            method.oauth2.as_ref().is_some_and(|spec| {
                spec.grants
                    .contains(&connector_spec::OAuthGrant::AuthorizationCode)
            })
        });
        // ...and the client SECRET belongs only to the *confidential* half of that leg (C-556). A
        // public PKCE client — Anthropic's Console OAuth — authenticates the exchange with PKCE and
        // no secret, so requiring an operator to supply one would collect a value nothing uses. The
        // discriminator is per credential, so a connector needs the secret only if some
        // `authorization_code` client of it is not public.
        let confidential_redirect = granted.iter().any(|method| {
            method.oauth2.as_ref().is_some_and(|spec| {
                spec.grants
                    .contains(&connector_spec::OAuthGrant::AuthorizationCode)
                    && !spec.public_client
            })
        });
        if redirects {
            assert_eq!(
                level_of("oauth.client_id"),
                Some((Some(connector_spec::Level::Operator), false)),
                "providers/{name}.toml declares an `authorization_code` grant without a public, operator-level client id"
            );
            // The third half of the registration (C-531). A grant whose redirect URI cannot be
            // supplied is one only a loopback deployment can complete, and `OAuth2Spec::redirect`
            // models nothing else — so without this a hosted host has nowhere to put its callback. A
            // public client still runs the browser leg, so it owes this one too.
            assert_eq!(
                level_of("oauth.redirect_uri"),
                Some((Some(connector_spec::Level::Operator), false)),
                "providers/{name}.toml declares an `authorization_code` grant without an operator-level, public redirect URI"
            );
        }
        if confidential_redirect {
            assert_eq!(
                level_of("oauth.client_secret"),
                Some((Some(connector_spec::Level::Operator), true)),
                "providers/{name}.toml declares a confidential `authorization_code` grant without an operator-level, secret client secret. A public PKCE client is exempt (`public_client = true`); a confidential one is not"
            );
        }

        for method in granted {
            let spec = method.oauth2.as_ref().expect("filtered above");
            assert!(
                !spec.token_path.is_empty(),
                "providers/{name}.toml: credential {:?} declares a grant with no token endpoint, \
                 which every grant and every refresh POSTs to",
                method.name
            );
            assert!(
                !spec
                    .grants
                    .contains(&connector_spec::OAuthGrant::AuthorizationCode)
                    || !spec.authorize_path.is_empty(),
                "providers/{name}.toml: credential {:?} allows the `authorization_code` grant and \
                 declares no authorize path, so nothing says where the browser leg begins",
                method.name
            );
            assert!(
                spec.endpoint.is_empty()
                    || connector.service_names().contains(&spec.endpoint.as_str()),
                "providers/{name}.toml: credential {:?} resolves its grant against endpoint {:?}, \
                 which is not a declared service — a grant naming an endpoint nothing declares \
                 reaches a host the allow-list never admitted. Leaving it empty is the other legal \
                 answer, and means the connector's own base URL",
                method.name,
                spec.endpoint
            );
            assert_ne!(
                method.subject,
                connector_spec::Subject::Unstated,
                "providers/{name}.toml: credential {:?} is obtained by a grant but does not say \
                 whose authority it carries, so a host cannot bound its reach (C-528)",
                method.name
            );
        }
    }

    assert!(checked > 0, "no providers were checked");
    // Derived-set discipline: if the corpus ever stops declaring OAuth, this test passes vacuously
    // and the archetype silently stops being exercised again.
    assert!(
        !with_oauth.is_empty(),
        "no shipped provider declares `[auth.oauth2]`, so archetype 7 is unexercised once more"
    );
}

/// An OAuth2 fixture with `authorization_code`, optionally a public PKCE client, declaring a
/// `client_id` config field and — deliberately — no `client_secret`. No shipped provider is public
/// yet (that is C-555 round 2), so the public/confidential split can only be exercised over fixtures.
fn oauth_fixture(public_client: bool) -> String {
    let discriminator = if public_client {
        "public_client = true\n"
    } else {
        ""
    };
    format!(
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"

[[auth]]
name = "acme.oauth"
scheme = "bearer"
env = ["ACME_ACCESS_TOKEN"]
subject = "user"

[auth.oauth2]
authorize_path = "/oauth/authorize"
token_path = "/oauth/token"
client_id = ""
grants = ["authorization_code"]
{discriminator}
[[operations]]
id = "acme-ping"
method = "GET"
direction = "read"
path = "/ping"
risk = "low"
idempotency = "idempotent"

[[config]]
name = "client_id"
label = "Client ID"
help = "From the app you registered with Acme"
binds = "oauth.client_id"
"#
    )
}

/// **The client secret is required of a confidential client and waived for a public one** (C-556).
///
/// The walk above requires an operator-level, secret `oauth.client_secret` field of every
/// `authorization_code` connector. That held only because every OAuth connector shipped so far is
/// confidential. A public PKCE client (Anthropic's Console OAuth) authenticates the exchange with
/// PKCE and no secret, so it must declare its `client_id` but not be required to publish a secret —
/// proven here over fixtures, because no shipped provider is public yet.
///
/// The discriminator is read through the **wire form** — `serde_json` — so this also pins that the
/// flag is what a document consumer sees, absent (confidential) by default and present only when
/// declared.
#[test]
fn a_public_client_is_exempt_from_the_client_secret_a_confidential_one_owes() {
    fn is_public(connector: &Connector) -> bool {
        connector.auth.iter().any(|method| {
            serde_json::to_value(method)
                .ok()
                .and_then(|value| value["oauth2"]["public_client"].as_bool())
                .unwrap_or(false)
        })
    }
    // The walk test's rule, stated once over the discriminator: a confidential `authorization_code`
    // client owes a client secret, a public one does not.
    fn client_secret_required(connector: &Connector) -> bool {
        connector.auth.iter().any(|method| {
            let public = serde_json::to_value(method)
                .ok()
                .and_then(|value| value["oauth2"]["public_client"].as_bool())
                .unwrap_or(false);
            method.oauth2.as_ref().is_some_and(|spec| {
                spec.grants
                    .contains(&connector_spec::OAuthGrant::AuthorizationCode)
            }) && !public
        })
    }
    fn declares_client_secret(connector: &Connector) -> bool {
        connector
            .config
            .iter()
            .any(|field| field.binds == "oauth.client_secret")
    }

    let public = provider::load("providers/fixture.toml", &oauth_fixture(true))
        .expect("a public-client fixture must load")
        .connector;
    let confidential = provider::load("providers/fixture.toml", &oauth_fixture(false))
        .expect("a confidential-client fixture must load")
        .connector;

    assert!(
        is_public(&public),
        "the public fixture must publish `public_client = true`"
    );
    assert!(
        !is_public(&confidential),
        "a confidential client omits the discriminator — absent is the default"
    );

    // Neither fixture declares a client secret. For the public client that is complete; for the
    // confidential one it is exactly the omission the walk test above must still catch.
    assert!(!declares_client_secret(&public));
    assert!(!declares_client_secret(&confidential));
    assert!(
        !client_secret_required(&public),
        "a public PKCE client must not be required to publish a client secret"
    );
    assert!(
        client_secret_required(&confidential),
        "a confidential `authorization_code` client without a client secret is still incomplete"
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
direction = "read"
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
