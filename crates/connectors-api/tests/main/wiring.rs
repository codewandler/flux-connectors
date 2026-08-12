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
//! # What freshdesk stands in for, and what it stopped standing in for (C-235)
//!
//! Freshdesk declares no credential because its API key occupies the Basic *username* position and
//! the IR cannot yet mark that secret — `AGENTS.md` records it as an intentional gap. It was
//! therefore the right fixture for the **shape** (a connector with nothing for an operator to
//! supply) and the wrong one for the **reason**, and it was served as `no-credential-required`
//! because the embedded catalogue could not carry the difference.
//!
//! It can now, so freshdesk is served as `no-credential` — its own state, and the honest one: there
//! is nothing to supply *and* the calls do not work. The genuinely-public case — C-206's
//! `auth = []`, declared positively — has still not shipped, so it is proved against fixtures in
//! `api.rs`'s own unit tests, where a declaration can be written down rather than waited for.

use crate::support::{client, serve, sign_in, Idp};

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
/// unset are opposite answers to "is there anything for me to do here?". Before C-212 they were the
/// same byte: `connected: false` for both.
///
/// The assertion is deliberately on a field of its own rather than on a difference a consumer would
/// have to *derive* — C-212's second acceptance item is that nobody has to correlate a boolean with
/// `credentials.length` to recover the state that was collapsed.
///
/// **C-235 moved freshdesk's token**, from `no-credential-required` to `no-credential`. The
/// property this test is named for is unchanged and is what is still asserted: a connector with
/// nothing to supply is not served as one left unset. What changed is that "nothing to supply" is
/// no longer one state — see
/// [`a_withheld_credential_is_not_served_as_a_vendor_that_needs_none`].
#[tokio::test]
async fn a_connector_needing_no_credential_is_not_served_as_one_left_unset() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let nothing_to_supply = view(&client, &base, &cookie, "freshdesk").await;
    let something_to_supply = view(&client, &base, &cookie, "anthropic").await;

    assert_eq!(
        nothing_to_supply["wiring"], "no-credential",
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

/// **C-235, over the real HTTP surface: the reason is served, not inferred.**
///
/// Freshdesk's nine operations name no credential, and until this story that was the only thing the
/// host could see — so it served them as a vendor requiring none, which reads to an operator as
/// *ready to use*. Every call 401s. The embedded catalogue now carries what the connector declares,
/// and the host publishes it per operation as well as per connector.
///
/// The genuinely-public half of the distinction is not asserted here because no connector ships it;
/// `api.rs`'s unit tests hold that half against fixtures. What *is* assertable over shipped data is
/// the half that was being told wrongly, which is the one an operator was meeting.
#[tokio::test]
async fn a_withheld_credential_is_not_served_as_a_vendor_that_needs_none() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let withheld = view(&client, &base, &cookie, "freshdesk").await;

    assert_eq!(
        withheld["wiring"], "no-credential",
        "freshdesk's credential is withheld, not unnecessary — `no-credential-required` told an \
         operator a connector that 401s on every call was ready: {withheld}"
    );
    assert_eq!(
        operation(&withheld, "freshdesk-ticket-list")["requirement"],
        "no-credential",
        "the reason is carried per operation, in the catalogue's own token: {withheld}"
    );
    assert_eq!(
        operation(&withheld, "freshdesk-ticket-list")["requires"],
        serde_json::json!([]),
        "and the mechanism list is unchanged — the reason travels beside it, not inside it"
    );
    assert_eq!(
        operation(&withheld, "freshdesk-ticket-list")["callable"],
        false,
        "an unauthenticated request to an endpoint that wants a credential is a 401"
    );
    assert_eq!(
        withheld["callable_operations"], 0,
        "none of freshdesk's operations is callable, and the count must say so: {withheld}"
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
/// Anthropic's eleven operations are callable with `api_key` alone, and the nine admin ones say for
/// themselves that they are not. C-441 widened the admin service from three reads to nine, which
/// moved the denominator and left the numerator alone — which is the property this test is for.
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
        after["operation_count"], 11,
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
        serde_json::json!([["anthropic.admin_key"], ["anthropic.console_oauth_admin"]]),
        "the per-operation mapping must name what is missing, as alternatives of mechanisms. \
         C-555 added the org:admin OAuth token beside the Admin API key, so this operation now has \
         two ways to become callable and the mapping must offer both rather than naming only the \
         one a host happens to prefer"
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
