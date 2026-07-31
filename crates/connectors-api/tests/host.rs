//! What this host must get right, asserted over its real HTTP surface.
//!
//! # Why there is no live vendor leg here
//!
//! Every test below stops at or before the send, and that is deliberate. To assert *"the request
//! that reached the vendor carried tenant A's credential"* a test needs a vendor it controls, which
//! means a loopback address — and no shipped connector's `base_url` can be pointed at one. Nine
//! carry a `{placeholder}`, but every one of them templates a *label* inside a fixed vendor suffix
//! (`{subdomain}.zendesk.com`), never the whole host. There is no configuration value that makes a
//! connector call `127.0.0.1`, by design.
//!
//! The available alternative was a substitute `Egress` that records instead of sending. That is
//! precisely what `connector-pack`'s own tests already do, for want of a transport, and `Egress`'s
//! documentation says what is wrong with treating it as proof: *"a stand-in that ignores `body`, or
//! that resolves `url` against some base of its own, is not a substitute — it is a different
//! connector."* Adding a second stubbed suite here would grow the count of green tests without
//! growing what is known.
//!
//! So the live leg is **manual and labelled manual**, which is the standard
//! `docs/designs/connectors-app.md` sets and the mistake
//! [`C-149`](../../../docs/stories/C-149-vault-live-leg-reports-ok-when-it-skips.md) records — a live
//! leg that reports OK when it skips is worse than none. It was performed against
//! `api.anthropic.com` on 2026-07-31 and is recorded in `crates/connectors-api/README.md`.
//!
//! What *is* asserted here is everything that happens before the socket, which is where this crate's
//! own defects would live: the address a credential resolves at, the tenant that address belongs to,
//! and whether a value can reach a surface.

use std::net::{Ipv4Addr, SocketAddr};

use connectors_api::App;

/// An obviously-fake credential. Nothing here may commit a value shaped like a real token — the same
/// care `connector-pack`'s own `SENTINEL` takes, and long enough that flux's redactor will hold it
/// (`Redactor::add_secret` silently ignores anything under six trimmed characters).
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-connectors-api";

/// Start the host on an ephemeral loopback port and return its base URL.
async fn serve() -> String {
    let app = App::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .expect("an ephemeral loopback port");
    let address = listener.local_addr().expect("a bound address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, connectors_api::router(app)).await;
    });
    format!("http://{address}")
}

/// **A credential value must not appear on any surface this host serves.**
///
/// The failure this prevents is not exotic, it is the natural shape of a convenience: an endpoint
/// that returns what it just stored so the page can show it, or an error that quotes the value it
/// could not accept. Asserted over *every* response body rather than over the one that looked
/// risky, because the point is that no route grows the habit.
#[tokio::test]
async fn a_stored_credential_reaches_no_surface() {
    let base = serve().await;
    let client = reqwest::Client::new();

    let stored = client
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .json(&serde_json::json!({ "value": SENTINEL }))
        .send()
        .await
        .expect("the store call completes");
    assert_eq!(stored.status(), 204);
    assert!(
        !stored.text().await.expect("a body").contains(SENTINEL),
        "the store response echoed the credential back"
    );

    for path in [
        "/v1/connectors",
        "/v1/connectors/anthropic",
        "/v1/operations/anthropic-models-list",
        "/",
    ] {
        let body = client
            .get(format!("{base}{path}"))
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

    // And the connector reports itself connected without ever saying with what.
    let view: serde_json::Value = client
        .get(format!("{base}/v1/connectors/anthropic"))
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
        api_key["address"], "tenants/local/com.anthropic.api/api_key",
        "the address is what an operator needs, and is not a secret"
    );
}

/// **Without a credential, the request is not sent, and the refusal names the address.**
///
/// The alternative — sending unauthenticated and letting the vendor answer `401` — is the failure
/// mode `connector-pack::Error::MissingCredential` exists to prevent: a host treating `401` as
/// retryable loops against the vendor forever without ever being told what is missing.
#[tokio::test]
async fn an_operation_without_its_credential_refuses_by_address() {
    let base = serve().await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!(
            "{base}/v1/operations/anthropic-models-list/execute"
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("the execute call completes");

    assert_eq!(response.status(), 400);
    let body: serde_json::Value = response.json().await.expect("json");
    let error = body["error"].as_str().expect("an error message");

    assert!(
        error.contains("tenants/local/com.anthropic.api/api_key"),
        "the refusal must name the address an operator has to fill: {error}"
    );
    assert!(
        error.contains("the request was not sent"),
        "the refusal must say the request was not sent: {error}"
    );
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
