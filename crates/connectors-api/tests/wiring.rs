//! **"Is there anything for me to do here?"** — asserted over the host's own HTTP surface.
//!
//! The question a single `connected` boolean could not answer. Three situations collapsed into two
//! values, and the two that collapsed together are opposites: *supply something* and *there is
//! nothing to supply* both served as `false`.
//!
//! # Why the assertions name connectors rather than measure the catalogue
//!
//! `AGENTS.md` — "A per-provider test asserts about its provider, never about the catalogue" — puts
//! a premise about **specific** connectors in the file that loads them **by name**, so that only
//! those connectors changing can falsify it. That is what this file does: `freshdesk` is the one
//! shipped connector declaring no credential, and `anthropic` is the one whose two credentials
//! belong to two different surfaces. Nothing here walks `providers/` or counts the catalogue, so a
//! fifty-fourth connector cannot turn it red merely by existing.
//!
//! If freshdesk gains a credential — C-16 owns that decision — this file goes red, and correctly:
//! the evidence it stands on stopped being true.
//!
//! # What freshdesk does and does not stand in for
//!
//! Freshdesk declares no credential because its API key occupies the Basic *username* position and
//! the IR cannot yet mark that secret — `AGENTS.md` records it as an intentional gap. It is
//! therefore the right fixture for the **shape** (a connector with nothing for an operator to
//! supply) and the wrong one for the **reason**. The genuinely-public case — C-206's
//! `auth = []`, declared positively — has not shipped, so it is proved against fixtures in
//! `api.rs`'s own unit tests, where a mechanism list can be written down rather than waited for.

mod support;

use support::{client, serve, sign_in, Idp};

/// The subject every test here signs in as. The same one `tests/host.rs` uses.
const OPERATOR: &str = "110169484474386276334";

/// An obviously-fake credential, long enough for flux's redactor to hold.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-connectors-api-wiring";

/// `GET /v1/connectors/<id>` as this session's tenant.
async fn view(client: &reqwest::Client, base: &str, cookie: &str, id: &str) -> serde_json::Value {
    client
        .get(format!("{base}/v1/connectors/{id}"))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap_or_else(|error| panic!("GET /v1/connectors/{id}: {error}"))
        .json()
        .await
        .unwrap_or_else(|error| panic!("GET /v1/connectors/{id} served no JSON: {error}"))
}

/// One operation's entry in a connector view.
fn operation<'v>(view: &'v serde_json::Value, id: &str) -> &'v serde_json::Value {
    view["operations"]
        .as_array()
        .unwrap_or_else(|| panic!("the view carries no `operations` array: {view}"))
        .iter()
        .find(|operation| operation["id"] == id)
        .unwrap_or_else(|| panic!("the view carries no operation `{id}`"))
}

/// **The third state is served, and it is not the second one.**
///
/// A connector with nothing for an operator to supply and a connector whose credentials are simply
/// unset are opposite answers to "is there anything for me to do here?". Before this story they
/// were the same byte: `connected: false` for both.
///
/// The assertion is deliberately on a field of its own rather than on a difference a consumer would
/// have to *derive* — the story's second acceptance item is that nobody has to correlate a boolean
/// with `credentials.length` to recover the state that was collapsed.
#[tokio::test]
async fn a_connector_needing_no_credential_is_not_served_as_one_left_unset() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let nothing_to_supply = view(&client, &base, &cookie, "freshdesk").await;
    let something_to_supply = view(&client, &base, &cookie, "anthropic").await;

    assert_eq!(
        nothing_to_supply["wiring"], "no-credential-required",
        "a connector whose operations declare no credential must say so positively, in the token \
         C-206 published for exactly this distinction: {nothing_to_supply}"
    );
    assert_eq!(
        something_to_supply["wiring"], "not-wired",
        "a connector whose credentials are unset must read as work to do: {something_to_supply}"
    );
    assert_ne!(
        nothing_to_supply["wiring"], something_to_supply["wiring"],
        "the two opposite situations are still served identically"
    );
}

/// **Supplying the credential an operation uses makes that operation callable.**
///
/// The second half of the same defect, measured against a running host: `all_stored` required
/// *every* declared credential, so storing Anthropic's `api_key` — which the model surface uses —
/// left the connector reading as unwired for want of `admin_key`, which belongs to the management
/// surface and which no ordinary request carries.
///
/// The unit here is the **operation**, so the answer is a count rather than a boolean: two of
/// Anthropic's five operations are callable with `api_key` alone, and the three admin ones say for
/// themselves that they are not.
#[tokio::test]
async fn supplying_one_credential_makes_the_operations_that_use_it_callable() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let before = view(&client, &base, &cookie, "anthropic").await;
    assert_eq!(before["callable_operations"], 0, "nothing is stored yet");

    let stored = client
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", &cookie)
        .json(&serde_json::json!({ "value": SENTINEL }))
        .send()
        .await
        .expect("the store call completes");
    assert_eq!(stored.status(), 204);

    let after = view(&client, &base, &cookie, "anthropic").await;
    assert_eq!(
        after["wiring"], "partly-wired",
        "storing the credential nearly every operation uses must move the connector off \
         `not-wired`: {after}"
    );
    assert_eq!(
        after["callable_operations"], 2,
        "`anthropic.api_key` is what `anthropic-models-list` and `anthropic-model-get` declare, \
         and nothing else: {after}"
    );
    assert_eq!(
        after["operation_count"], 5,
        "the denominator is the connector's own operation count"
    );

    assert_eq!(
        operation(&after, "anthropic-models-list")["callable"],
        true,
        "the operation whose credential is stored must say it is callable"
    );
    assert_eq!(
        operation(&after, "anthropic-organization-get")["callable"],
        false,
        "an operation whose credential is unset must say it is not callable"
    );
    assert_eq!(
        operation(&after, "anthropic-organization-get")["requires"],
        serde_json::json!([["anthropic.admin_key"]]),
        "the per-operation mapping must name what is missing, as alternatives of mechanisms"
    );
}

/// **The new fields carry no credential value, on any path including an error.**
///
/// `tests/host.rs::a_stored_credential_reaches_no_surface` is the guarantee and is unchanged; this
/// re-proves it over the surface this story added, because a per-operation view is exactly where
/// the convenient mistake — "show what is stored so the page can render it" — would live.
#[tokio::test]
async fn the_wiring_surface_never_carries_a_credential_value() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let stored = client
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", &cookie)
        .json(&serde_json::json!({ "value": SENTINEL }))
        .send()
        .await
        .expect("the store call completes");
    assert_eq!(stored.status(), 204);

    // The served views, now that a value exists to leak.
    for path in [
        "/v1/connectors",
        "/v1/connectors/anthropic",
        "/v1/connectors/freshdesk",
    ] {
        let body = client
            .get(format!("{base}{path}"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap_or_else(|error| panic!("GET {path}: {error}"))
            .text()
            .await
            .expect("a body");
        assert!(
            !body.contains(SENTINEL),
            "`{path}` served the credential value"
        );
    }

    // And the error paths. A refusal is where a value is most likely to be quoted back.
    for (path, cookie_header) in [
        ("/v1/connectors/anthropic", None),
        ("/v1/connectors/no-such-connector", Some(cookie.as_str())),
        (
            "/v1/connectors/anthropic",
            Some("connectors_session=not-a-session"),
        ),
    ] {
        let mut request = client.get(format!("{base}{path}"));
        if let Some(header) = cookie_header {
            request = request.header("cookie", header);
        }
        let body = request
            .send()
            .await
            .unwrap_or_else(|error| panic!("GET {path}: {error}"))
            .text()
            .await
            .expect("a body");
        assert!(
            !body.contains(SENTINEL),
            "`{path}` served the credential value on an error"
        );
    }
}
