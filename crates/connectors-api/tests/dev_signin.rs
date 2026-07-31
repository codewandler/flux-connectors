//! **The dev sign-in: a second door, and everything it is not allowed to be.**
//!
//! C-234 adds a way into this host that needs no Google registration, because until it existed the
//! only door was a real OAuth client and a developer without one could not reach a single `/v1`
//! route. That is auth-bypass code, so the tests here are mostly about what it must *not* do.
//!
//! The five properties, and why each is asserted over HTTP rather than over the code that
//! implements it:
//!
//! 1. **The route does not exist without `--dev`.** Not "exists and refuses" — absent. A `403` is a
//!    decision taken at request time by a handler somebody could later edit; a `404` is the router
//!    never having been given the path. Asserted by building the plain host exactly as
//!    `cargo run -p connectors-api` builds it and asking for the route.
//! 2. **The session it mints is the ordinary kind.** Same `Set-Cookie` attributes, same opacity,
//!    same `Principal` extraction, same server-side revocation — because a dev mode that
//!    special-cases the session type makes every other route behave differently under test than in
//!    production. These assertions are deliberately the same ones `tests/tenancy.rs` makes about a
//!    Google session.
//! 3. **The dev tenant cannot collide with a real one**, and no credential crosses between them.
//! 4. **The invariant `tests/host.rs` exists to keep still holds under `--dev`**: no credential
//!    value reaches any served surface, including on error.
//! 5. **The door cannot be pushed open from another origin.**
//!
//! Nothing here touches C-204's login-CSRF binding. The dev door stands beside it.

mod support;

use std::net::{Ipv4Addr, SocketAddr};

use base64::Engine as _;
use connectors_api::App;
use support::{client, cookie_named, env_lock, set_cookie_headers, sign_in, Idp};

/// An obviously-fake credential, in the same shape `tests/host.rs` uses and for the same reason:
/// nothing in this repository may commit a value shaped like a real token.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-connectors-api-dev";

/// The tenant a dev session owns. Written out rather than read from the crate, so that a change to
/// the derivation is a failing test rather than a test that agrees with whatever the code now does.
const DEV_TENANT: &str = "dev-local";

/// The Google subject used in the one test where both doors are open at once.
const OPERATOR: &str = "110169484474386276334";
const OPERATOR_TENANT: &str = "google-110169484474386276334";

// ---------------------------------------------------------------------------------------------
// Hosts
// ---------------------------------------------------------------------------------------------

/// A host with **no** Google registration and **no** dev door — a first run, exactly as it is
/// today.
async fn serve_plain() -> String {
    serve(unconfigured_app()).await
}

/// A host with no Google registration and the dev door open — `cargo run -p connectors-api --
/// --dev` in an empty environment, which is the case this story exists for.
async fn serve_dev() -> String {
    serve(unconfigured_app().with_dev_signin()).await
}

/// Build an `App` with Google deliberately unconfigured.
///
/// The environment is per process and read once at construction, so the guard is held across
/// "clear, then build" for the same reason `support::serve` holds it across "set, then build". It
/// is dropped before anything is awaited.
fn unconfigured_app() -> App {
    let _guard = env_lock();
    std::env::remove_var(connectors_api::auth::oidc::CLIENT_ID_ENV);
    std::env::remove_var(connectors_api::auth::oidc::CLIENT_SECRET_ENV);
    App::new(env!("CARGO_MANIFEST_DIR")).expect("an unconfigured host still starts")
}

/// Build an `App` pointed at `idp` **and** with the dev door open, so both doors stand together.
fn google_and_dev_app(idp: &Idp) -> App {
    let _guard = env_lock();
    std::env::set_var(
        connectors_api::auth::oidc::CLIENT_ID_ENV,
        support::CLIENT_ID,
    );
    std::env::set_var(
        connectors_api::auth::oidc::CLIENT_SECRET_ENV,
        support::CLIENT_SECRET,
    );
    std::env::set_var(
        connectors_api::auth::oidc::REDIRECT_URI_ENV,
        "http://127.0.0.1/auth/callback",
    );
    std::env::set_var("CONNECTORS_OIDC_ISSUER", &idp.issuer);
    std::env::set_var("CONNECTORS_OIDC_AUTHORIZE_URL", idp.authorize_url());
    std::env::set_var("CONNECTORS_OIDC_TOKEN_URL", idp.token_url());
    std::env::set_var("CONNECTORS_OIDC_JWKS_URL", idp.jwks_url());
    App::new(env!("CARGO_MANIFEST_DIR"))
        .expect("the crate root exists")
        .with_dev_signin()
}

/// Serve one `App` on an ephemeral loopback port and return its base URL.
async fn serve(app: App) -> String {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .expect("an ephemeral loopback port");
    let address = listener.local_addr().expect("a bound address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, connectors_api::router(app)).await;
    });
    format!("http://{address}")
}

/// Complete a dev sign-in and return the session cookie's `name=value`.
async fn sign_in_as_dev(base: &str, browser: &reqwest::Client) -> String {
    let response = browser
        .post(format!("{base}/auth/dev"))
        .send()
        .await
        .expect("the dev sign-in call completes");
    assert_eq!(
        response.status(),
        303,
        "the dev sign-in must establish a session and send the browser on, like the Google callback"
    );
    support::session_cookie(&response).expect("the dev sign-in set a session cookie")
}

// ---------------------------------------------------------------------------------------------
// 1. The route exists only with the flag
// ---------------------------------------------------------------------------------------------

/// **The failing-first test.** The dev door is absent from a plain host and open on a `--dev` one.
///
/// Both halves live in one test on purpose. Asserting only the `404` would pass on a host that
/// never grew the route at all — which is every host that existed before this story — and a test
/// that cannot fail is not evidence. Pairing it with the `--dev` half makes the assertion the real
/// one: *this exact path is reachable when and only when the flag is on.*
///
/// The `404` matters more than it looks. A route that exists and answers `403` is one edited
/// condition away from answering `200`, and that edit would read like a refactor in a diff. A route
/// the router was never given cannot be reached by a misconfiguration at all, which is why the flag
/// is consulted where the route table is built and nowhere else.
#[tokio::test]
async fn the_dev_sign_in_route_exists_only_under_the_dev_flag() {
    let browser = client();

    // --- without the flag ---------------------------------------------------------------------
    let plain = serve_plain().await;
    let refused = browser
        .post(format!("{plain}/auth/dev"))
        .send()
        .await
        .expect("the call completes");
    assert_eq!(
        refused.status(),
        404,
        "the dev sign-in route answered on a host started without --dev; it must not exist there, \
         and 403 is not good enough — an absent route cannot be reached by a misconfiguration"
    );
    assert!(
        support::session_cookie(&refused).is_none(),
        "a host without --dev handed out a session cookie"
    );
    // And the door it would have opened is still shut, which is the state C-234 is about.
    assert_eq!(
        browser
            .get(format!("{plain}/v1/connectors"))
            .send()
            .await
            .expect("the call completes")
            .status(),
        401,
        "a plain host must still refuse every /v1 route without a session"
    );

    // --- with the flag ------------------------------------------------------------------------
    let dev = serve_dev().await;
    let cookie = sign_in_as_dev(&dev, &browser).await;

    let connectors = browser
        .get(format!("{dev}/v1/connectors"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("the call completes");
    assert_eq!(
        connectors.status(),
        200,
        "a dev session did not open the routes it exists to open"
    );
    assert!(
        !connectors
            .json::<serde_json::Value>()
            .await
            .expect("json")
            .as_array()
            .expect("an array of connectors")
            .is_empty(),
        "a dev session saw an empty catalogue"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. It is the ordinary kind of session
// ---------------------------------------------------------------------------------------------

/// **A dev session is the same kind of thing as a Google session.**
///
/// The attribute list is copied deliberately from `tests/tenancy.rs`'s
/// `the_session_cookie_is_opaque_and_locked_down`, because "the same guarantees" is only meaningful
/// if it is the same assertions. If that test's list ever grows, this one is the reminder that the
/// dev door has to grow with it.
#[tokio::test]
async fn a_dev_session_carries_the_same_cookie_guarantees_as_a_google_one() {
    let browser = client();
    let base = serve_dev().await;

    let response = browser
        .post(format!("{base}/auth/dev"))
        .send()
        .await
        .expect("the call completes");
    let header = set_cookie_headers(&response)
        .into_iter()
        .find(|header| header.starts_with(connectors_api::auth::SESSION_COOKIE))
        .expect("the dev sign-in set a session cookie");

    let lowered = header.to_lowercase();
    assert!(lowered.contains("httponly"), "not HttpOnly: {header}");
    assert!(lowered.contains("secure"), "not Secure: {header}");
    assert!(
        lowered.contains("samesite=lax"),
        "not SameSite=Lax: {header}"
    );
    assert!(
        lowered.contains("max-age") || lowered.contains("expires"),
        "the dev session cookie never expires: {header}"
    );
    assert!(
        lowered.contains("path=/"),
        "the dev session cookie is not scoped to the whole host: {header}"
    );

    // Opaque: the token names neither the account nor the tenant, in the clear or base64-decoded.
    let value = header
        .split(';')
        .next()
        .and_then(|pair| pair.split_once('='))
        .map(|(_, value)| value.to_owned())
        .expect("a cookie value");
    assert!(
        !value.to_lowercase().contains("dev"),
        "the dev session cookie is not opaque; it says what it is: {value}"
    );
    for decoded in [
        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&value),
        base64::engine::general_purpose::STANDARD.decode(&value),
    ]
    .into_iter()
    .flatten()
    {
        let text = String::from_utf8_lossy(&decoded).to_string();
        assert!(
            !text.contains(DEV_TENANT) && !text.to_lowercase().contains("not a real account"),
            "the dev cookie decodes to account data: {text}"
        );
    }

    // Revocation is server-side for a dev session too, so signing out is real rather than cosmetic.
    let cookie = support::session_cookie(&response).expect("a session cookie");
    let stolen = cookie.clone();
    browser
        .post(format!("{base}/auth/signout"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("the sign-out completes");
    assert_eq!(
        browser
            .get(format!("{base}/v1/connectors"))
            .header("cookie", &stolen)
            .send()
            .await
            .expect("the call completes")
            .status(),
        401,
        "a copy of a signed-out dev session still worked, so dev sessions are a kind of their own"
    );
}

/// **The dev identity is unmistakably fake where an operator looks at it.**
///
/// `/auth/me` and the page header are where a person reads which account they are in. Neither may
/// show anything that could be mistaken for a real address — no `dev@example.com` styled to look
/// like a mailbox somebody owns.
#[tokio::test]
async fn the_dev_identity_is_unmistakably_fake() {
    let browser = client();
    let base = serve_dev().await;
    let cookie = sign_in_as_dev(&base, &browser).await;

    let me: serde_json::Value = browser
        .get(format!("{base}/auth/me"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("the call completes")
        .json()
        .await
        .expect("json");

    assert_eq!(
        me["email"],
        serde_json::Value::Null,
        "the dev account carries an email address, a label that could be mistaken for a real one"
    );
    let name = me["name"].as_str().expect("the dev account has a label");
    assert!(
        name.contains("NOT A REAL ACCOUNT"),
        "the dev account's label does not say it is fake: {name}"
    );
    assert_eq!(me["tenant"], DEV_TENANT);
    assert!(
        !me["subject"]
            .as_str()
            .expect("a subject")
            .chars()
            .all(|c| c.is_ascii_digit()),
        "the dev subject is shaped like a Google `sub`, which is a run of digits"
    );

    // The page reads `/auth/status` to decide whether to draw the button at all, so a plain host
    // must not advertise a button that would 404.
    let status: serde_json::Value = browser
        .get(format!("{base}/auth/status"))
        .send()
        .await
        .expect("the call completes")
        .json()
        .await
        .expect("json");
    assert_eq!(status["dev"], true, "a --dev host does not announce itself");

    let plain = serve_plain().await;
    let status: serde_json::Value = browser
        .get(format!("{plain}/auth/status"))
        .send()
        .await
        .expect("the call completes")
        .json()
        .await
        .expect("json");
    assert_eq!(
        status["dev"], false,
        "a plain host advertises a dev button that would 404"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Tenants cannot collide
// ---------------------------------------------------------------------------------------------

/// **A dev tenant and a Google tenant are disjoint, and both doors may stand open at once.**
///
/// The recorded decision (`docs/designs/connectors-api.md`) is that `--dev` is *not* refused when a
/// Google registration is configured. This is the test that has to hold for that to be defensible:
/// a credential stored by the signed-in Google operator is invisible to a dev session on the same
/// process, and the two addresses differ in their first path segment.
///
/// The collision argument is structural rather than statistical. `Account::from_claims` is the only
/// constructor reachable from a token and it always produces `google-{sub}`; `Account::developer`
/// always produces `dev-local`. Two literal prefixes, two constructors, one module, private fields.
#[tokio::test]
async fn a_dev_session_and_a_google_session_share_no_credential() {
    let idp = Idp::start().await;
    let base = serve(google_and_dev_app(&idp)).await;
    let browser = client();

    // The real door still works with the dev door open beside it.
    let google = sign_in(&base, OPERATOR).await;
    let stored = browser
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", &google)
        .json(&serde_json::json!({ "value": SENTINEL }))
        .send()
        .await
        .expect("the store call completes");
    assert_eq!(stored.status(), 204);

    let dev = sign_in_as_dev(&base, &browser).await;

    let dev_view: serde_json::Value = browser
        .get(format!("{base}/v1/connectors/anthropic"))
        .header("cookie", &dev)
        .send()
        .await
        .expect("the view")
        .json()
        .await
        .expect("json");
    let credential = dev_view["credentials"]
        .as_array()
        .expect("credentials")
        .iter()
        .find(|c| c["name"] == "anthropic.api_key")
        .expect("the declared credential");

    assert_eq!(
        credential["stored"], false,
        "a dev session sees the Google operator's credential as stored, so the tenants collided"
    );
    assert_eq!(
        credential["address"],
        format!("tenants/{DEV_TENANT}/com.anthropic.api/api_key"),
        "a dev session resolves credentials at the Google operator's address"
    );
    assert_ne!(
        credential["address"].as_str(),
        Some(format!("tenants/{OPERATOR_TENANT}/com.anthropic.api/api_key").as_str())
    );

    // And the Google session is unchanged by the dev session existing.
    let google_view: serde_json::Value = browser
        .get(format!("{base}/v1/connectors/anthropic"))
        .header("cookie", &google)
        .send()
        .await
        .expect("the view")
        .json()
        .await
        .expect("json");
    assert_eq!(
        google_view["credentials"]
            .as_array()
            .expect("credentials")
            .iter()
            .find(|c| c["name"] == "anthropic.api_key")
            .expect("the declared credential")["stored"],
        true,
        "the Google operator lost their credential when a dev session appeared"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. The one invariant this host exists to keep
// ---------------------------------------------------------------------------------------------

/// **Under `--dev`, no credential value reaches any served surface — including on error.**
///
/// This is `tests/host.rs`'s `a_stored_credential_reaches_no_surface`, re-proved through the dev
/// door. Dev mode gets its session by a different route; it must not get a different answer to the
/// question this host exists to answer. The sweep includes `/auth/dev` itself, which is a new
/// surface on the same origin and therefore exactly where the habit would start.
#[tokio::test]
async fn a_credential_stored_in_dev_mode_reaches_no_surface() {
    let base = serve_dev().await;
    let browser = client();
    let cookie = sign_in_as_dev(&base, &browser).await;
    let session_token = cookie.split_once('=').expect("name=value").1.to_owned();

    // Before anything is stored: the refusal an operator sees must name the *dev* address, so the
    // message is as useful under --dev as it is under a real sign-in.
    let refusal: serde_json::Value = browser
        .post(format!(
            "{base}/v1/operations/anthropic-models-list/execute"
        ))
        .header("cookie", &cookie)
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("the execute call completes")
        .json()
        .await
        .expect("json");
    let error = refusal["error"].as_str().expect("an error message");
    assert!(
        error.contains(&format!("tenants/{DEV_TENANT}/com.anthropic.api/api_key")),
        "the refusal must name the address an operator has to fill: {error}"
    );

    let stored = browser
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

    let secrets = [
        (SENTINEL, "the connector credential"),
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
        let body = browser
            .get(format!("{base}{path}"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap_or_else(|error| panic!("GET {path}: {error}"))
            .text()
            .await
            .expect("a body");
        for (secret, what) in secrets {
            assert!(!body.contains(secret), "`{path}` served {what} under --dev");
        }
    }

    // The error paths, where a value is most likely to be quoted back — and the dev route itself,
    // which is the new surface.
    for (path, method, cookie_header) in [
        ("/v1/connectors", "GET", None),
        (
            "/v1/connectors",
            "GET",
            Some("connectors_session=not-a-session"),
        ),
        ("/auth/me", "GET", None),
        ("/auth/dev", "POST", None),
        (
            "/v1/credentials/anthropic/nope",
            "PUT",
            Some(cookie.as_str()),
        ),
    ] {
        let url = format!("{base}{path}");
        let mut request = match method {
            "POST" => browser.post(url),
            "PUT" => browser
                .put(url)
                .json(&serde_json::json!({ "value": SENTINEL })),
            _ => browser.get(url),
        };
        if let Some(header) = cookie_header {
            request = request.header("cookie", header);
        }
        let body = request
            .send()
            .await
            .unwrap_or_else(|error| panic!("{method} {path}: {error}"))
            .text()
            .await
            .expect("a body");
        for (secret, what) in secrets {
            assert!(
                !body.contains(secret),
                "`{method} {path}` served {what} on an error"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 5. The door cannot be pushed from another origin
// ---------------------------------------------------------------------------------------------

/// **A cross-site `POST` cannot mint a dev session.**
///
/// This route mints a session from a request carrying no cookie, so `SameSite=Lax` — which is what
/// stops a third-party page driving the *credential-writing* routes — does nothing for it. A form
/// on `evil.example` auto-posting to `http://localhost:8787/auth/dev` would otherwise silently put
/// the operator's browser into the dev tenant. That is C-204's login-CSRF shape with a much smaller
/// blast radius (the dev tenant is local and holds only what was pasted into it), and it costs one
/// header check to close.
///
/// Defence in depth rather than the primary control — the primary controls are that the route does
/// not exist without `--dev` and that the host is loopback-only. `Sec-Fetch-Site` is sent by every
/// current browser and by no command-line client, so an absent header is treated as "not a
/// browser", which is what keeps `curl` usable for exactly the hand-verification this story asks
/// for. Nothing about C-204's binding is touched.
#[tokio::test]
async fn a_cross_site_post_cannot_mint_a_dev_session() {
    let browser = client();
    let base = serve_dev().await;

    let response = browser
        .post(format!("{base}/auth/dev"))
        .header("sec-fetch-site", "cross-site")
        .send()
        .await
        .expect("the call completes");

    assert_eq!(
        response.status(),
        403,
        "a cross-site POST minted a dev session"
    );
    assert!(
        support::session_cookie(&response).is_none(),
        "a cross-site POST was refused but still set a session cookie"
    );

    // A same-origin one, which is what the button sends, is honoured.
    let allowed = browser
        .post(format!("{base}/auth/dev"))
        .header("sec-fetch-site", "same-origin")
        .send()
        .await
        .expect("the call completes");
    assert_eq!(allowed.status(), 303);
    assert!(cookie_named(&allowed, connectors_api::auth::SESSION_COOKIE).is_some());
}
