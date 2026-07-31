//! What this host must get right, asserted over its real HTTP surface.
//!
//! # Why the live vendor leg is not in *this* file
//!
//! Every test below stops at or before the send. That used to be the whole story here, on the
//! reasoning that no shipped connector's `base_url` can be pointed at a loopback address — nine
//! carry a `{placeholder}`, but every one of them templates a *label* inside a fixed vendor suffix
//! (`{subdomain}.zendesk.com`), never the whole host, and C-214's `request::Slot` guard exists to
//! keep it that way. That reasoning still holds, and it is still why no test *drives a route* to a
//! vendor it controls.
//!
//! **What no longer holds is the conclusion that nothing here can send.** `tests/live_egress.rs`
//! (C-202) sends one request through the real `HttpRequestTool` to a loopback server and asserts
//! the vendor received exactly the `{ method, url, headers, body }` the pack built. It reaches a
//! loopback address by retargeting one string literal in a shipped operation's own emitted Flux
//! and by granting `PrivateNetAllow::Hosts(["127.0.0.1"])` through [`App::with_web_options`] — a
//! grant for one host, on one `App`, with the shipped default untouched. That file documents the
//! trade in full.
//!
//! The alternative both files refuse is a substitute `Egress` that records instead of sending —
//! precisely what `connector-pack`'s own tests do, for want of a transport, and `Egress`'s
//! documentation says what is wrong with treating it as proof: *"a stand-in that ignores `body`, or
//! that resolves `url` against some base of its own, is not a substitute — it is a different
//! connector."*
//!
//! The leg against a **real vendor** stays manual and labelled manual, which is the standard
//! `docs/designs/connectors-app.md` sets and the mistake
//! [`C-149`](../../../docs/stories/C-149-vault-live-leg-reports-ok-when-it-skips.md) records — a live
//! leg that reports OK when it skips is worse than none. It was performed against
//! `api.anthropic.com` on 2026-07-31 and is recorded in `crates/connectors-api/README.md`.
//!
//! What *is* asserted here is everything that happens before the socket, which is where this crate's
//! own defects would live: the address a credential resolves at, the tenant that address belongs to,
//! and whether a value can reach a surface.

mod support;

use connectors_api::App;
use support::{client, serve, sign_in, Idp};

/// An obviously-fake credential. Nothing here may commit a value shaped like a real token — the same
/// care `connector-pack`'s own `SENTINEL` takes, and long enough that flux's redactor will hold it
/// (`Redactor::add_secret` silently ignores anything under six trimmed characters).
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-connectors-api";

/// The subject every test here signs in as.
const OPERATOR: &str = "110169484474386276334";

/// The tenant that subject resolves to. Written out rather than computed, so that a change to the
/// derivation is a failing test rather than a test that agrees with whatever the code now does.
const OPERATOR_TENANT: &str = "google-110169484474386276334";

/// **A credential value must not appear on any surface this host serves.**
///
/// The failure this prevents is not exotic, it is the natural shape of a convenience: an endpoint
/// that returns what it just stored so the page can show it, or an error that quotes the value it
/// could not accept. Asserted over *every* response body rather than over the one that looked
/// risky, because the point is that no route grows the habit.
///
/// C-204 widened it in three directions and the widening is the interesting part. **The sign-in
/// routes are swept too**, because they are new surfaces on the same origin and the natural shape
/// of a convenience there is the same one. **The Google client secret is swept alongside the
/// connector credential**, because it is a credential this host now holds and a token endpoint
/// that echoed the request back — several do — would put it in an error body. And **the session
/// token is swept**, because a response that reflected it would turn any response-reflection bug
/// into session theft.
#[tokio::test]
async fn a_stored_credential_reaches_no_surface() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;
    let session_token = cookie.split_once('=').expect("name=value").1.to_owned();

    let stored = client
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", &cookie)
        .json(&serde_json::json!({ "value": SENTINEL }))
        .send()
        .await
        .expect("the store call completes");
    assert_eq!(stored.status(), 204);
    assert!(
        !stored.text().await.expect("a body").contains(SENTINEL),
        "the store response echoed the credential back"
    );

    // Every secret this host now holds, and every surface it serves.
    let secrets = [
        (SENTINEL, "the connector credential"),
        (support::CLIENT_SECRET, "the Google client secret"),
        (session_token.as_str(), "the session token"),
    ];

    for path in [
        "/v1/connectors",
        "/v1/connectors/anthropic",
        "/v1/operations/anthropic-models-list",
        "/auth/me",
        "/auth/status",
        "/",
    ] {
        let response = client
            .get(format!("{base}{path}"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap_or_else(|error| panic!("GET {path}: {error}"));
        // **The sweep must run against the real surface, not an error page** (C-228). Every path
        // here is fetched *with* a session precisely so it renders the thing that could leak. A
        // refusal body contains no secret either, so without this line the guarantee would still be
        // reported as held if a route started answering `401` — which is how `/v1/operations/…`
        // came to be swept here while having no test that its gate was even reachable.
        let status = response.status();
        assert!(
            status.is_success(),
            "GET {path} answered {status} with a session, so the no-secrets sweep below would pass \
             on an error page rather than on the surface it claims to cover"
        );
        let body = response.text().await.expect("a body");
        for (secret, what) in secrets {
            assert!(!body.contains(secret), "`{path}` served {what}");
        }
    }

    // The error paths too. A refusal is where a value is most likely to be quoted back, and the
    // routes below are reached without a session, with a bad session, and with a bad payload.
    for (path, cookie_header) in [
        ("/v1/connectors", None),
        ("/v1/connectors", Some("connectors_session=not-a-session")),
        ("/auth/me", None),
        (
            "/auth/callback?code=x&state=never-issued",
            Some(cookie.as_str()),
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
        for (secret, what) in secrets {
            assert!(!body.contains(secret), "`{path}` served {what} on an error");
        }
    }

    // And the connector reports itself connected without ever saying with what.
    let view: serde_json::Value = client
        .get(format!("{base}/v1/connectors/anthropic"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("the view")
        .json()
        .await
        .expect("json");
    let api_key = view["credentials"]
        .as_array()
        .expect("credentials")
        .iter()
        .find(|c| c["name"] == "anthropic.api_key")
        .expect("the declared credential");
    assert_eq!(api_key["stored"], true, "the value was stored");
    assert_eq!(
        api_key["address"],
        format!("tenants/{OPERATOR_TENANT}/com.anthropic.api/api_key"),
        "the address is the signed-in account's, and is not a secret"
    );
}

/// **Without a credential, the request is not sent, and the refusal names the address.**
///
/// The alternative — sending unauthenticated and letting the vendor answer `401` — is the failure
/// mode `connector-pack::Error::MissingCredential` exists to prevent: a host treating `401` as
/// retryable loops against the vendor forever without ever being told what is missing.
#[tokio::test]
async fn an_operation_without_its_credential_refuses_by_address() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let response = client
        .post(format!(
            "{base}/v1/operations/anthropic-models-list/execute"
        ))
        .header("cookie", &cookie)
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("the execute call completes");

    assert_eq!(response.status(), 400);
    let body: serde_json::Value = response.json().await.expect("json");
    let error = body["error"].as_str().expect("an error message");

    assert!(
        error.contains(&format!(
            "tenants/{OPERATOR_TENANT}/com.anthropic.api/api_key"
        )),
        "the refusal must name the address an operator has to fill: {error}"
    );
    assert!(
        error.contains("the request was not sent"),
        "the refusal must say the request was not sent: {error}"
    );
}

/// **A host with no Google registration starts, and says what is missing.**
///
/// This is the first-run path, and it is the one most likely to be got wrong in the direction that
/// wastes an afternoon. Panicking at startup turns `cargo run -p connectors-api` into a stack
/// trace; starting silently turns it into a sign-in button that leads nowhere. Neither tells an
/// operator that two environment variables are unset, so both are refused here: the host binds,
/// serves its page, and every sign-in surface answers `503` with the variable names and the
/// console URL to register them at.
#[tokio::test]
async fn without_a_google_registration_the_host_still_starts_and_explains_itself() {
    use std::net::{Ipv4Addr, SocketAddr};

    let app = {
        // Held across "clear the environment, build the `App`" for the same reason `support::serve`
        // holds it across "set the environment, build the `App`".
        let _guard = support::env_lock();
        std::env::remove_var(connectors_api::auth::oidc::CLIENT_ID_ENV);
        std::env::remove_var(connectors_api::auth::oidc::CLIENT_SECRET_ENV);
        App::new(env!("CARGO_MANIFEST_DIR")).expect("an unconfigured host still starts")
    };

    let message = app
        .setup_message()
        .expect("an unconfigured host says what is missing");
    assert!(message.contains(connectors_api::auth::oidc::CLIENT_ID_ENV));
    assert!(message.contains(connectors_api::auth::oidc::CLIENT_SECRET_ENV));
    assert!(
        message.contains("console.cloud.google.com"),
        "the message must say where to register: {message}"
    );
    assert!(
        message.contains("/auth/callback"),
        "the message must name the redirect URI to register: {message}"
    );

    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .expect("an ephemeral loopback port");
    let base = format!("http://{}", listener.local_addr().expect("a bound address"));
    tokio::spawn(async move {
        let _ = axum::serve(listener, connectors_api::router(app)).await;
    });
    let client = client();

    // The page still renders, which is what an operator is looking at.
    assert_eq!(
        client
            .get(&base)
            .send()
            .await
            .expect("the page loads")
            .status(),
        200,
        "an unconfigured host served no page"
    );

    // And sign-in refuses in a way that names the fix.
    let response = client
        .get(format!("{base}/auth/signin"))
        .send()
        .await
        .expect("the call completes");
    assert_eq!(
        response.status(),
        503,
        "an unconfigured sign-in must refuse"
    );
    let body = response.text().await.expect("a body");
    assert!(
        body.contains(connectors_api::auth::oidc::CLIENT_ID_ENV),
        "the refusal does not name what is missing: {body}"
    );

    // `/auth/status` is the machine-readable half, so the page can render the same thing.
    let status: serde_json::Value = client
        .get(format!("{base}/auth/status"))
        .send()
        .await
        .expect("the call completes")
        .json()
        .await
        .expect("json");
    assert_eq!(status["configured"], false);
    assert_eq!(status["signed_in"], false);
    assert!(status["setup"]
        .as_str()
        .is_some_and(|setup| setup.contains(connectors_api::auth::oidc::CLIENT_ID_ENV)));
}

/// **The transport is flux's own `http.request`, not something this crate wrote.**
///
/// `Egress` is typed as `dyn Tool` and so cannot enforce what it holds — its own documentation names
/// this as the consequence it accepts: *"a wrongly-wired host would send every connector's traffic
/// somewhere else."* Nothing in the type system closes that, so it is closed here.
#[test]
fn the_transport_is_flux_http_request() {
    let app = App::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
    assert_eq!(
        app.egress().tool().spec().name,
        "http.request",
        "this host is wired to a transport that is not flux's http.request"
    );
}

/// **The default egress refuses private and loopback hosts.**
///
/// `WebOptions::default()` carries `PrivateNetAllow::None`, the full SSRF guard. A host that quietly
/// shipped `Any` would let a connector — or a prompt-injected caller choosing a parameter that lands
/// in a URL — reach into the network the host is running on. Asserted on the value rather than
/// trusted from the doc comment, because the default is one word away from the opposite.
///
/// **What this does *not* assert, measured** (C-202): it reads a constant out of `flux-web`, not
/// this host's policy. Changing [`App::new`] to pass `PrivateNetAllow::Any` leaves this test green —
/// tried, and it stays green — because `WebOptions::default()` is unchanged by that edit. The gap is
/// closed behaviourally by `tests/live_egress.rs`'s
/// `the_default_egress_refuses_the_very_request_the_grant_admits`, which runs a real request under
/// [`App::new`] and requires it to be refused with nothing on the wire. Both are kept: this one
/// names the value a reader is looking for, that one proves the host actually uses it.
#[test]
fn the_default_egress_guards_the_private_network() {
    use flux_system::net::PrivateNetAllow;

    assert!(
        matches!(
            flux_web::WebOptions::default().private_net,
            PrivateNetAllow::None
        ),
        "the default egress policy admits private addresses"
    );
}
