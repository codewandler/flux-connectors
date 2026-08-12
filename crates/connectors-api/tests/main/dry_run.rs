//! **"Why will this not work?"**, answered before anything is sent (C-237, over C-145's seam).
//!
//! The operator console's Send button has two failure modes that look identical on the page: the
//! vendor refused the call, and the call was never buildable. `POST /v1/operations/{id}/dry-run`
//! separates them by building the request and stopping — [`connector_pack::Operation::dry_run`],
//! which holds no client and therefore cannot reach a socket whatever it is asked to do.
//!
//! # Why this file exists rather than a sentence in the story
//!
//! C-237 lists the dry-run panel as *"optional but high value"* and attaches a condition to it:
//! *"Verify that it refuses usefully for an unbound configuration before committing to the panel's
//! copy; that has not been run."* The panel's copy tells an operator that a refusal here names the
//! missing fact. That is a claim about a diagnostic, and the only way to hold a diagnostic to its
//! wording is to assert on it.
//!
//! # Why the assertions name connectors
//!
//! `AGENTS.md` — "a per-provider test asserts about its provider, never about the catalogue".
//! `zendesk` is loaded by name because it is a connector whose base URL carries a `{subdomain}` an
//! operator must bind, and `anthropic` because its base URL is literal and its credential is an
//! ordinary header. Nothing here walks the catalogue, so a fifty-fifth connector cannot turn it red.

use crate::support::{client, serve, sign_in, Idp};

/// The subject every test here signs in as. The same one `tests/host.rs` uses.
const OPERATOR: &str = "110169484474386276334";

/// `POST /v1/operations/{id}/dry-run`, with the status it answered.
async fn rehearse(
    client: &reqwest::Client,
    base: &str,
    cookie: &str,
    operation: &str,
) -> (reqwest::StatusCode, serde_json::Value) {
    let response = client
        .post(format!("{base}/v1/operations/{operation}/dry-run"))
        .header("cookie", cookie)
        .json(&serde_json::json!({ "ticket_id": "42" }))
        .send()
        .await
        .unwrap_or_else(|error| panic!("POST /v1/operations/{operation}/dry-run: {error}"));
    let status = response.status();
    let body = response
        .json()
        .await
        .unwrap_or_else(|error| panic!("the dry run served no JSON: {error}"));
    (status, body)
}

/// **An unbound configuration field is named, along with the service it belongs to.**
///
/// The refusal an operator actually hits first, and the one the panel's copy promises to make
/// legible. Zendesk's base URL is `https://{subdomain}.zendesk.com`, and until that is bound the
/// request is not a degraded call — it is a request to a different host — so the pack refuses it
/// rather than putting a brace on the wire. What this asserts is that the refusal survives the trip
/// through the host: `Failure`'s `{error:#}` renders the whole chain, which is where the pack's own
/// diagnostic lives.
#[tokio::test]
async fn an_unbound_configuration_field_is_refused_by_name() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let (status, body) = rehearse(&client, &base, &cookie, "zendesk-ticket-show").await;
    assert_eq!(
        status, 400,
        "an unbuildable request was not refused: {body}"
    );

    let message = body["error"]
        .as_str()
        .unwrap_or_else(|| panic!("the refusal carries no `error` string: {body}"));
    assert!(
        message.contains("subdomain"),
        "the refusal does not name the field an operator has to bind, so the panel's copy \
         promises a diagnostic it does not deliver: {message}"
    );
    assert!(
        message.contains("zendesk-ticket-show"),
        "the refusal does not name the operation it is about: {message}"
    );
    assert!(
        message.contains("default"),
        "the refusal does not name the service the field belongs to — which is the first path \
         segment of the PUT that fixes it: {message}"
    );
}

/// **A buildable operation rehearses without a stored credential, and carries a reference.**
///
/// The property that makes this safe to offer beside a Send button: `dry_run` places each declared
/// credential's *reference* rather than its value, so it answers identically over an empty store
/// and there is no path from this route to a secret. Nothing is stored by this test, and the
/// rehearsed request still shows exactly where the credential would go.
#[tokio::test]
async fn a_rehearsal_shows_the_request_and_never_a_value() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let (status, body) = rehearse(&client, &base, &cookie, "anthropic-models-list").await;
    assert_eq!(
        status, 200,
        "a buildable operation did not rehearse: {body}"
    );

    assert_eq!(body["operation"], "anthropic-models-list");
    assert_eq!(body["tool"], "anthropic.models.list");
    assert_eq!(body["request"]["method"], "GET");
    assert!(
        body["request"]["url"]
            .as_str()
            .is_some_and(|url| url.starts_with("https://api.anthropic.com")),
        "the rehearsed request does not name the host it would reach: {}",
        body["request"]["url"]
    );

    let credentials = body["credentials"]
        .as_array()
        .unwrap_or_else(|| panic!("the rehearsal carries no credentials array: {body}"));
    assert_eq!(
        credentials.len(),
        1,
        "the rehearsal does not show the one credential this call would carry: {body}"
    );
    assert_eq!(credentials[0]["credential"], "anthropic.api_key");
    let reference = credentials[0]["reference"]
        .as_str()
        .expect("a reference string");
    assert!(
        !reference.is_empty(),
        "the rehearsed credential stands in for nothing at all"
    );
    // The whole document, not just the credential entry: a reference that leaked into a header
    // would be a value that could.
    assert!(
        body.to_string().contains(reference),
        "the reference does not appear on the rehearsed request, so the panel cannot show where \
         the credential goes"
    );
}
