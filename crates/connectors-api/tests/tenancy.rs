//! **The tenant comes from the session, and from nothing else.**
//!
//! This is the confused-deputy seam, and it is the one property in this crate whose failure is
//! silent. A host that reads the tenant from a path segment, a body field or a header answers a
//! request with authority the caller did not have, and it does so with a `200` — which is exactly
//! the shape [`connectors-proxy.md`](../../../docs/designs/connectors-proxy.md) rejected and which
//! its successor answers by making *the caller the principal whose credential is being used*.
//!
//! So the assertion is deliberately adversarial rather than merely positive: the request below
//! names tenant B four different ways while carrying tenant A's session, and the credential must
//! land at **A**.
//!
//! Every test here drives the real HTTP surface against a real loopback identity provider. Nothing
//! constructs a session through a Rust back door, because a back door is precisely the thing whose
//! absence is being asserted.

mod support;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Value;
use sha2::{Digest, Sha256};
use support::{client, query_param, serve, session_cookie, sign_in, Idp};

/// **A request naming tenant B while carrying tenant A's session resolves to A.**
///
/// The credential is stored through A's session while the request says "B" in every place a host
/// might be tempted to look — a body field, two headers, and a query parameter. Then both tenants
/// are asked what they have. A must hold it and B must not, and the second half is the half that
/// matters: a host keying everything on one constant passes the first assertion and fails this one.
#[tokio::test]
async fn a_request_naming_another_tenant_resolves_to_the_session_s_tenant() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();

    let alice = sign_in(&base, "alice-1").await;
    let bob = sign_in(&base, "bob-2").await;
    assert_ne!(alice, bob, "two sign-ins produced the same session token");

    // Tenant B is named four ways at once, and none of them may be believed.
    let stored = client
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", &alice)
        .header("x-tenant", "google-bob-2")
        .header("x-tenant-id", "google-bob-2")
        .query(&[("tenant", "google-bob-2")])
        .json(&serde_json::json!({
            "value": "SENTINEL-NOT-A-REAL-SECRET-alice-anthropic-key",
            "tenant": "google-bob-2",
        }))
        .send()
        .await
        .expect("the store call completes");
    assert_eq!(
        stored.status(),
        204,
        "storing under A's session must succeed"
    );

    let alice_view = connector_view(&client, &base, &alice).await;
    let bob_view = connector_view(&client, &base, &bob).await;

    let alice_key = credential(&alice_view, "anthropic.api_key");
    let bob_key = credential(&bob_view, "anthropic.api_key");

    assert_eq!(
        alice_key["stored"], true,
        "the credential did not land in the session's own tenant"
    );
    assert_eq!(
        bob_key["stored"], false,
        "tenant B received a credential stored under tenant A's session — the tenant was read \
         from the request, not from the session"
    );

    let alice_address = alice_key["address"].as_str().expect("A has an address");
    let bob_address = bob_key["address"].as_str().expect("B has an address");
    assert_ne!(
        alice_address, bob_address,
        "two accounts resolved to one credential address"
    );
    assert!(
        !alice_address.contains("bob-2"),
        "the request's tenant reached the address: {alice_address}"
    );
    assert!(
        alice_address.contains("alice-1"),
        "the address is not derived from the signed-in subject: {alice_address}"
    );
}

/// **Without a session there is no tenant, so there is no answer.**
///
/// The alternative — falling back to a default tenant — is how an unauthenticated caller reads and
/// writes whichever account that default happens to be.
#[tokio::test]
async fn an_unauthenticated_request_is_refused() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();

    for (method, path) in [
        ("GET", "/v1/connectors"),
        ("GET", "/v1/connectors/anthropic"),
        ("PUT", "/v1/credentials/anthropic/anthropic.api_key"),
        ("POST", "/v1/operations/anthropic-models-list/execute"),
    ] {
        let request = match method {
            "GET" => client.get(format!("{base}{path}")),
            "PUT" => client
                .put(format!("{base}{path}"))
                .json(&serde_json::json!({ "value": "x" })),
            _ => client
                .post(format!("{base}{path}"))
                .json(&serde_json::json!({})),
        };
        let response = request.send().await.expect("the call completes");
        assert_eq!(
            response.status(),
            401,
            "{method} {path} answered without a session"
        );
    }
}

/// **Sign-out revokes server-side, so a stolen cookie stops working.**
///
/// A client-side expiry — `Max-Age=0` and nothing else — logs out the browser that asked and
/// leaves the copy an attacker took working until it expires on its own.
#[tokio::test]
async fn signing_out_revokes_the_session_for_a_stolen_copy() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();

    let session = sign_in(&base, "carol-3").await;
    // The attacker's copy: the same bytes, held elsewhere.
    let stolen = session.clone();

    assert_eq!(
        client
            .get(format!("{base}/v1/connectors"))
            .header("cookie", &stolen)
            .send()
            .await
            .expect("the call completes")
            .status(),
        200,
        "the session did not work before sign-out"
    );

    let out = client
        .post(format!("{base}/auth/signout"))
        .header("cookie", &session)
        .send()
        .await
        .expect("the sign-out completes");
    assert!(
        out.status().is_success() || out.status().is_redirection(),
        "sign-out failed: {}",
        out.status()
    );

    assert_eq!(
        client
            .get(format!("{base}/v1/connectors"))
            .header("cookie", &stolen)
            .send()
            .await
            .expect("the call completes")
            .status(),
        401,
        "a copy of the cookie still worked after sign-out — revocation is client-side only"
    );
}

/// **The session cookie is `HttpOnly`, `Secure`, `SameSite=Lax`, and carries no credential.**
///
/// Asserted on the header the host actually sends rather than on the code that builds it. The
/// token itself must also be opaque — the account it belongs to is server-side state, not
/// something a holder can read out of their own cookie.
#[tokio::test]
async fn the_session_cookie_is_opaque_and_locked_down() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();

    let begun = support::begin_sign_in(&base, &client).await;
    let callback = client
        .get(format!("{base}/auth/callback"))
        .header("cookie", &begun.login_cookie)
        .query(&[
            ("code", format!("dave-4|{}", begun.nonce)),
            ("state", begun.state),
        ])
        .send()
        .await
        .expect("the callback completes");
    assert_eq!(callback.status(), 303, "the sign-in did not complete");

    // The *session* cookie specifically. The callback sets two, and picking "the first
    // `Set-Cookie`" would have this assert against the cleared login cookie — which carries every
    // attribute checked below and an empty value, so it would pass while proving nothing.
    let header = support::set_cookie_headers(&callback)
        .into_iter()
        .find(|header| header.starts_with(connectors_api::auth::SESSION_COOKIE))
        .expect("the callback set a session cookie");

    let lowered = header.to_lowercase();
    assert!(lowered.contains("httponly"), "not HttpOnly: {header}");
    assert!(lowered.contains("secure"), "not Secure: {header}");
    assert!(
        lowered.contains("samesite=lax"),
        "not SameSite=Lax: {header}"
    );
    assert!(
        lowered.contains("max-age") || lowered.contains("expires"),
        "the session cookie never expires: {header}"
    );

    // Opaque: the value decodes to no readable claim about the account.
    let value = header
        .split(';')
        .next()
        .and_then(|pair| pair.split_once('='))
        .map(|(_, value)| value.to_owned())
        .expect("a cookie value");
    assert!(
        !value.contains("dave-4"),
        "the cookie names the subject: {value}"
    );
    for decoded in [
        URL_SAFE_NO_PAD.decode(&value).ok(),
        base64::engine::general_purpose::STANDARD
            .decode(&value)
            .ok(),
    ]
    .into_iter()
    .flatten()
    {
        let text = String::from_utf8_lossy(&decoded).to_string();
        assert!(
            !text.contains("dave-4") && !text.contains("example.test"),
            "the cookie decodes to account data: {text}"
        );
    }
}

/// **The host proves possession of the PKCE verifier, and the challenge really is S256 of it.**
///
/// PKCE asserted by reading the authorize URL alone would prove only that a parameter was spelled;
/// this checks that what the token endpoint received is the pre-image of what consent was asked
/// with. It also pins `code_challenge_method`, because `plain` is a legal value that defeats it.
#[tokio::test]
async fn the_authorization_code_is_bound_to_a_pkce_verifier() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();

    let start = client
        .get(format!("{base}/auth/signin"))
        .send()
        .await
        .expect("sign-in starts");
    let authorize = start
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("a Location header")
        .to_owned();

    assert_eq!(
        query_param(&authorize, "code_challenge_method").as_deref(),
        Some("S256"),
        "the authorize URL does not ask for S256 PKCE: {authorize}"
    );
    assert_eq!(
        query_param(&authorize, "response_type").as_deref(),
        Some("code"),
        "the flow is not authorization-code: {authorize}"
    );
    assert!(
        !authorize.contains(support::CLIENT_SECRET),
        "the authorize URL carries the client secret: {authorize}"
    );
    let challenge = query_param(&authorize, "code_challenge").expect("a challenge");
    let state = query_param(&authorize, "state").expect("a state");
    let nonce = query_param(&authorize, "nonce").expect("a nonce");
    let binding = support::cookie_named(&start, connectors_api::auth::LOGIN_COOKIE)
        .expect("sign-in binds the flow to this browser");

    let completed = client
        .get(format!("{base}/auth/callback"))
        .header("cookie", &binding)
        .query(&[("code", format!("erin-5|{nonce}")), ("state", state)])
        .send()
        .await
        .expect("the callback completes");
    assert_eq!(
        completed.status(),
        303,
        "the sign-in did not complete, so the exchange below would assert about nothing"
    );

    let requests = idp.token_requests.lock().expect("not poisoned").clone();
    let form = requests.last().expect("the host exchanged the code");
    let verifier = form
        .iter()
        .find(|(key, _)| key == "code_verifier")
        .map(|(_, value)| value.clone())
        .expect("the token request carried a code_verifier");

    assert_eq!(
        URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        challenge,
        "the challenge consent was asked with is not S256 of the verifier that redeemed the code"
    );
    assert_eq!(
        form.iter()
            .find(|(key, _)| key == "grant_type")
            .map(|(_, value)| value.as_str()),
        Some("authorization_code"),
        "the exchange was not an authorization_code grant"
    );
}

/// **A `state` issued to one browser cannot be redeemed by another.**
///
/// # The attack this reproduces
///
/// Login CSRF, and it is the severe direction of it: not "the attacker signs in as the victim" but
/// **"the victim is silently signed in as the attacker"**. The attacker begins a sign-in in their
/// own browser, keeps the `state`, and gets the victim's browser to fetch the callback URL — a
/// top-level `GET`, so an `<img>`, a redirect or a link is enough. The victim lands on a working,
/// signed-in page and has no way to tell it is not theirs. Every credential they then paste is
/// written to the **attacker's** tenant, where the attacker reads it back and runs operations with
/// it.
///
/// # Why the first version of this crate was vulnerable, and why its tests did not say so
///
/// The pending login lived in a process-global map keyed only by the `state`, with nothing tying an
/// entry to the user-agent that began the flow, and `/auth/signin` set no cookie at all. Any browser
/// presenting any live `state` could redeem it.
///
/// The guard test below this one — `a_callback_with_an_unknown_state_is_refused` — could not see
/// it, because it only ever presents a state that was **never issued**. Single-use consumption is a
/// *replay* defence and was mistaken for a CSRF defence; they are different properties and both are
/// needed. RFC 6749 §10.12 is explicit that the binding value must be kept *"in a location
/// accessible only to the client and the user-agent"* — which means a cookie.
///
/// So the assertion here is on **the identity that results**, not on a status code. The failure
/// mode being guarded against is a `303` followed by a perfectly healthy `200` belonging to
/// somebody else.
#[tokio::test]
async fn a_state_issued_to_one_browser_cannot_be_redeemed_by_another() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;

    // The attacker begins a sign-in in their own browser and keeps what it hands them. Read
    // straight off the redirect rather than through `support::begin_sign_in`, so that this test
    // exercises the vulnerability itself and does not depend on the fix's own cookie existing.
    let attacker = client();
    let start = attacker
        .get(format!("{base}/auth/signin"))
        .send()
        .await
        .expect("sign-in starts");
    let authorize = start
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("a Location header")
        .to_owned();
    let attacker_state = query_param(&authorize, "state").expect("a state");
    let attacker_nonce = query_param(&authorize, "nonce").expect("a nonce");

    // The victim's browser. Fresh: it never called `/auth/signin` and carries no cookie of ours.
    let victim = client();
    let callback = victim
        .get(format!("{base}/auth/callback"))
        .query(&[
            ("code", format!("ATTACKER|{attacker_nonce}")),
            ("state", attacker_state),
        ])
        .send()
        .await
        .expect("the callback completes");

    assert_ne!(
        callback.status(),
        303,
        "the victim's browser completed a sign-in it never started"
    );
    assert!(
        session_cookie(&callback).is_none(),
        "the victim's browser was handed a session for a flow it never began"
    );

    // The decisive assertion: whatever the victim is now holding, it is not an identity. Checked
    // by asking the host, with every cookie the exchange produced, rather than by trusting the
    // status code — a 200 with the wrong subject is the whole failure mode.
    let carried = support::set_cookie_headers(&callback)
        .iter()
        .filter_map(|header| header.split(';').next())
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("; ");

    let me = victim
        .get(format!("{base}/auth/me"))
        .header("cookie", &carried)
        .send()
        .await
        .expect("the identity call completes");
    assert_eq!(
        me.status(),
        401,
        "the victim's browser resolves to an identity: {}",
        me.text().await.unwrap_or_default()
    );

    // And it cannot write a credential into anybody's tenant.
    let stored = victim
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", &carried)
        .json(&serde_json::json!({ "value": "SENTINEL-NOT-A-REAL-SECRET-victim-paste" }))
        .send()
        .await
        .expect("the store call completes");
    assert_eq!(
        stored.status(),
        401,
        "the victim wrote a credential into a tenant that is not theirs"
    );

    // The attacker's own browser holds nothing either: beginning a flow is not being signed in.
    let attacker_cookies = support::set_cookie_headers(&start)
        .iter()
        .filter_map(|header| header.split(';').next())
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("; ");
    let attacker_me = attacker
        .get(format!("{base}/auth/me"))
        .header("cookie", &attacker_cookies)
        .send()
        .await
        .expect("the identity call completes");
    assert_eq!(
        attacker_me.status(),
        401,
        "beginning a sign-in alone resolved to an identity"
    );
}

/// **The cookie that binds a sign-in to a browser is as hardened as the session cookie.**
///
/// It is short-lived and scoped to the callback, because it has one job and a ten-minute life.
/// `SameSite=Lax` rather than `Strict` is deliberate and load-bearing: the callback arrives as a
/// **cross-site top-level GET** from the identity provider, and `Strict` would withhold the cookie
/// on exactly that request — turning the fix into a sign-in that never completes.
#[tokio::test]
async fn the_login_cookie_is_scoped_short_lived_and_locked_down() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let browser = client();

    let start = browser
        .get(format!("{base}/auth/signin"))
        .send()
        .await
        .expect("sign-in starts");

    let header = support::set_cookie_headers(&start)
        .into_iter()
        .find(|header| header.starts_with(connectors_api::auth::LOGIN_COOKIE))
        .expect("GET /auth/signin sets a login cookie");

    let lowered = header.to_lowercase();
    assert!(lowered.contains("httponly"), "not HttpOnly: {header}");
    assert!(lowered.contains("secure"), "not Secure: {header}");
    assert!(
        lowered.contains("samesite=lax"),
        "the callback is a cross-site top-level GET; Strict would drop this cookie: {header}"
    );
    assert!(
        lowered.contains("path=/auth/callback"),
        "the login cookie is not scoped to the callback: {header}"
    );
    assert!(
        lowered.contains("max-age="),
        "the login cookie never expires: {header}"
    );
}

/// **A completed sign-in clears the login cookie.**
///
/// Left behind, it is a spent binding value sitting in the browser for its full ten minutes.
#[tokio::test]
async fn completing_a_sign_in_clears_the_login_cookie() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let browser = client();

    let begun = support::begin_sign_in(&base, &browser).await;
    let callback = browser
        .get(format!("{base}/auth/callback"))
        .header("cookie", &begun.login_cookie)
        .query(&[
            ("code", format!("frank-7|{}", begun.nonce)),
            ("state", begun.state),
        ])
        .send()
        .await
        .expect("the callback completes");

    assert_eq!(callback.status(), 303, "the good path still works");
    assert!(
        session_cookie(&callback).is_some(),
        "the good path still sets a session"
    );

    let cleared = support::set_cookie_headers(&callback)
        .into_iter()
        .find(|header| header.starts_with(connectors_api::auth::LOGIN_COOKIE))
        .expect("the callback addresses the login cookie");
    assert!(
        cleared.to_lowercase().contains("max-age=0"),
        "the spent login cookie was not cleared: {cleared}"
    );
}

/// **A callback carrying a login cookie that disagrees with the `state` is refused.**
///
/// The cookie is present, so this is not the "no cookie at all" case; it is the attacker who can
/// make the victim's browser hold *some* login cookie — a second tab, a stale flow — and then
/// supplies their own `state` in the URL. Both halves must come from the same sign-in.
#[tokio::test]
async fn a_callback_whose_cookie_and_state_disagree_is_refused() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;

    let attacker = client();
    let victim = client();
    let attackers = support::begin_sign_in(&base, &attacker).await;
    let victims = support::begin_sign_in(&base, &victim).await;

    // The victim's own live cookie, the attacker's live state.
    let callback = victim
        .get(format!("{base}/auth/callback"))
        .header("cookie", &victims.login_cookie)
        .query(&[
            ("code", format!("ATTACKER|{}", attackers.nonce)),
            ("state", attackers.state),
        ])
        .send()
        .await
        .expect("the callback completes");

    assert_eq!(
        callback.status(),
        400,
        "a login cookie from one flow redeemed another flow's state"
    );
    assert!(
        session_cookie(&callback).is_none(),
        "a mismatched callback established a session"
    );
}

/// **A callback whose `state` was not issued by this host is refused.**
///
/// This is login-CSRF: an attacker who can make a browser fetch a callback URL of their choosing
/// signs the victim into the *attacker's* account, and everything the victim then connects is
/// connected for the attacker.
#[tokio::test]
async fn a_callback_with_an_unknown_state_is_refused() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();

    let response = client
        .get(format!("{base}/auth/callback"))
        .query(&[
            ("code", "mallory-6|whatever"),
            ("state", "a-state-this-host-never-issued"),
        ])
        .send()
        .await
        .expect("the callback completes");

    assert_eq!(
        response.status(),
        400,
        "an unsolicited callback was accepted"
    );
    assert!(
        session_cookie(&response).is_none(),
        "a refused callback still set a session cookie"
    );
}

/// One connector, as this session sees it.
async fn connector_view(client: &reqwest::Client, base: &str, cookie: &str) -> Value {
    client
        .get(format!("{base}/v1/connectors/anthropic"))
        .header("cookie", cookie)
        .send()
        .await
        .expect("the view call completes")
        .json()
        .await
        .expect("json")
}

/// One declared credential out of a connector view.
fn credential<'a>(view: &'a Value, name: &str) -> &'a Value {
    view["credentials"]
        .as_array()
        .unwrap_or_else(|| panic!("no credentials in {view}"))
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap_or_else(|| panic!("no credential `{name}` in {view}"))
}
