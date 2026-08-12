//! **Whose authority does this credential carry?** — C-528's declaration and its fail-closed default.
//!
//! Slack forces the axis. One OAuth v2 grant returns two tokens in one response: `access_token` is
//! the workspace's bot and `authed_user.access_token` is the signed-in person. They are placed
//! identically (`Authorization: Bearer …`), acquired by the same grant, and differ in nothing any
//! other axis can express — while differing entirely in who they act as and how much they reach.
//!
//! The default is `unstated` rather than `app`, and that is the substance of the design. 55
//! connectors ship credentials that nobody has reviewed for this, and at least one is genuinely
//! ambiguous today: `github.token` is documented as covering both a GitHub App installation token
//! and a personal access token, which are opposite answers. A default of `app` would record that the
//! question was never asked while reading as though it had been answered.

use connector_spec::{Connector, Subject};

fn provider(auth: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
base_url = "https://api.acme.example"
description = "A provider that exists to be checked."
{auth}

[[operations]]
id = "acme-thing-list"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List the things."
risk = "low"
idempotency = "idempotent"
"#
    )
}

fn load(source: &str) -> connector_spec::Result<Connector> {
    connector_spec::provider::load("providers/acme.toml", source).map(|loaded| loaded.connector)
}

/// A credential that says nothing about its subject is **unstated**, never `app`.
#[test]
fn an_undeclared_subject_is_unstated() {
    let connector = load(&provider(
        r#"
[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]
"#,
    ))
    .expect("a plain credential must load");

    assert_eq!(
        connector.auth[0].subject,
        Subject::Unstated,
        "an unreviewed credential must not claim a subject"
    );
}

/// Both real answers round-trip, and Slack's two-token shape is expressible on one connector.
#[test]
fn a_connector_declares_a_bot_and_a_user_credential_side_by_side() {
    let connector = load(&provider(
        r#"
[[auth]]
name = "acme.bot_token"
scheme = "bearer"
env = ["ACME_BOT_TOKEN"]
subject = "app"

[[auth]]
name = "acme.user_token"
scheme = "bearer"
env = ["ACME_USER_TOKEN"]
subject = "user"
"#,
    ))
    .expect("two subjects on one connector must load");

    assert_eq!(connector.auth[0].subject, Subject::App);
    assert_eq!(connector.auth[1].subject, Subject::User);
    assert_eq!(Subject::App.word(), "app");
    assert_eq!(Subject::User.word(), "user");
    assert_eq!(Subject::Unstated.word(), "unstated");
}

/// **The published artifacts do not move for an unstated subject.**
///
/// This is what let the axis be added to 55 shipped connectors without rewriting a manifest: the
/// field is skipped when it carries the default, so a connector that declares nothing serializes
/// exactly as it did before.
#[test]
fn an_unstated_subject_serializes_to_nothing() {
    let connector = load(&provider(
        r#"
[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]
"#,
    ))
    .expect("a plain credential must load");

    let rendered = serde_json::to_string(&connector.auth[0]).expect("a credential serializes");
    assert!(
        !rendered.contains("subject"),
        "an unstated subject reached an artifact: {rendered}"
    );

    let stated = load(&provider(
        r#"
[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]
subject = "user"
"#,
    ))
    .expect("a stated credential must load");
    let rendered = serde_json::to_string(&stated.auth[0]).expect("a credential serializes");
    assert!(
        rendered.contains(r#""subject":"user""#),
        "a stated subject must reach the artifact: {rendered}"
    );
}
