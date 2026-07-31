//! The four routes sign-in needs.
//!
//! `GET /auth/signin` · `GET /auth/callback` · `POST /auth/signout` · `GET /auth/me`, plus
//! `GET /auth/status` so the page can say "not configured yet" instead of rendering a button that
//! cannot work.

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::api::Failure;
use crate::auth::oidc::{Expectations, Settings, Setup, VerifyError, SCOPES};
use crate::auth::session::Account;
use crate::auth::{cleared_cookie, session_cookie, token_of, Principal};
use crate::state::{App, Oidc};

/// **Start a sign-in.**
///
/// Mints `state`, `nonce` and a PKCE pair, keeps the verifier and the nonce here, and sends the
/// browser to Google with only the challenge. The code that comes back is therefore redeemable
/// only by whoever holds the verifier — this process — which is what PKCE buys even for a
/// confidential client whose secret could be stolen from a redirect.
///
/// It also **sets [`crate::auth::LOGIN_COOKIE`]**, which is what ties the flow to this browser.
/// Without it the `state` is a bare value redeemable by anybody who presents it, and the attack
/// that follows is login CSRF in its severe direction: the victim signed in as the attacker. See
/// [`crate::auth::login_cookie`] for the full account.
pub async fn signin(State(app): State<App>) -> Response {
    let Some(oidc) = app.oidc() else {
        return not_configured(&app);
    };

    let login = app.sessions().start_login();
    let binding = crate::auth::login_cookie(&login.state);
    // `oauth_authorize_url` reads only `challenge` from the pair. The verifier stays in the session
    // store and never reaches a URL, which is the entire point of the exchange.
    let pkce = flux_credentials::Pkce {
        verifier: String::new(),
        challenge: login.challenge,
    };
    let mut url = flux_credentials::oauth_authorize_url(
        &oidc.settings.authorize_url,
        &oidc.settings.client_id,
        &oidc.settings.redirect_uri,
        SCOPES,
        &pkce,
        &login.state,
    );
    // `oauth_authorize_url` is a generic RFC-6749 builder and OIDC's `nonce` is not one of its
    // parameters, so it is appended here. It is the claim that binds the returned `id_token` to
    // *this* sign-in; without it a token obtained in another session replays into this one.
    url.push_str(&format!("&nonce={}", urlencoding::encode(&login.nonce)));

    (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, url), (header::SET_COOKIE, binding)],
    )
        .into_response()
}

/// What Google sends back.
#[derive(Deserialize)]
pub struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    /// Present when the person declined, or the request was rejected.
    error: Option<String>,
}

/// **Finish a sign-in.**
///
/// Four things must hold before an account exists, and they close four different attacks:
///
/// 1. **The browser presents the binding cookie, and it matches the `state` in the URL.** This is
///    the login-CSRF defence — see below. Checked *first*, before the pending entry is even looked
///    up, so a cross-site attempt cannot consume somebody else's live `state` as a side effect of
///    being refused.
/// 2. **The `state` is one this host issued and has not already redeemed** (single-use). This is
///    the *replay* defence, and it is a different property from 1 — conflating the two is the
///    mistake that left this route exploitable.
/// 3. **The code is exchanged with the PKCE verifier**, which never left this process.
/// 4. **The `id_token` passes every check** in [`crate::auth::oidc`].
pub async fn callback(State(app): State<App>, request: axum::extract::Request) -> Response {
    let (parts, _) = request.into_parts();
    let params = match Query::<CallbackParams>::try_from_uri(&parts.uri) {
        Ok(Query(params)) => params,
        Err(error) => return refuse(format!("the callback's query is unreadable: {error}")),
    };

    let Some(oidc) = app.oidc() else {
        return not_configured(&app);
    };

    if let Some(error) = params.error {
        // The provider's own error code, which is a fixed vocabulary (`access_denied`, …) and
        // carries nothing of ours.
        return refuse(format!(
            "the identity provider refused the sign-in: {error}"
        ));
    }
    let (Some(code), Some(state)) = (params.code, params.state) else {
        return refuse("the callback carried no code and state".to_owned());
    };

    // ---------------------------------------------------------------------------------------
    // 1. Is this the browser that began the flow?
    // ---------------------------------------------------------------------------------------
    //
    // The `state` in the URL travels through the identity provider and is therefore visible to
    // whoever built the link. The cookie does not: only this origin can set it, and only the
    // browser it was set in sends it back. Requiring both, and requiring them to agree, is what
    // makes the URL alone useless — which is RFC 6749 §10.12's requirement that the binding value
    // live "in a location accessible only to the client and the user-agent".
    //
    // A missing cookie is refused rather than treated as "no binding to check". That distinction
    // is the entire vulnerability: an attacker's victim has no cookie, and a check that skips when
    // absent is a check that is never performed on the one request that matters.
    let Some(bound_state) = crate::auth::login_state_of(&parts) else {
        return refuse_and_clear(
            "this callback did not come from a browser that started a sign-in here".to_owned(),
        );
    };
    if !crate::auth::oidc::constant_time_eq(bound_state.as_bytes(), state.as_bytes()) {
        return refuse_and_clear(
            "this callback's state does not belong to this browser's sign-in".to_owned(),
        );
    }

    // ---------------------------------------------------------------------------------------
    // 2. Was it issued here, and not already spent?
    // ---------------------------------------------------------------------------------------
    let Some((verifier, nonce)) = app.sessions().take_login(&state) else {
        return refuse_and_clear(
            "this callback does not correspond to a sign-in this host started".to_owned(),
        );
    };

    // ---------------------------------------------------------------------------------------
    // 3 and 4. Redeem the code, and believe the token only after every check.
    // ---------------------------------------------------------------------------------------
    let id_token = match exchange(oidc, &code, &verifier).await {
        Ok(token) => token,
        Err(why) => return refuse_and_clear(why),
    };

    let expect = Expectations {
        issuers: &oidc.settings.issuers,
        audience: &oidc.settings.client_id,
        nonce: &nonce,
    };

    let claims = match verify_with_rotation(&app, &id_token, &expect).await {
        Ok(claims) => claims,
        Err(error) => return refuse_and_clear(error.to_string()),
    };

    let account = match Account::from_claims(&claims) {
        Ok(account) => account,
        Err(why) => return refuse_and_clear(format!("this identity cannot own a tenant: {why}")),
    };
    let account = app.accounts().of_subject(account);
    let token = app.sessions().create(account);

    // Two `Set-Cookie` headers: establish the session, and clear the spent binding. Appended
    // rather than handed to axum as an array, because an array of header pairs with one repeated
    // name is the shape where "insert" and "append" differ and only one of them is correct.
    let mut response =
        (StatusCode::SEE_OTHER, [(header::LOCATION, "/".to_owned())]).into_response();
    append_cookie(&mut response, session_cookie(&token));
    append_cookie(&mut response, crate::auth::cleared_login_cookie());
    response
}

/// Add one more `Set-Cookie` to a response without displacing those already on it.
fn append_cookie(response: &mut Response, cookie: String) {
    if let Ok(value) = axum::http::HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

/// Verify, and survive a key rotation.
///
/// An unknown `kid` is what a rotation looks like from here, so it earns exactly one forced
/// refetch — rate-limited inside the cache, so a stream of bogus `kid`s cannot turn this host into
/// a load generator pointed at Google.
async fn verify_with_rotation(
    app: &App,
    id_token: &str,
    expect: &Expectations<'_>,
) -> Result<crate::auth::oidc::IdClaims, VerifyError> {
    let oidc = app.oidc().expect("checked by the caller");

    let keys = oidc.jwks.current().await?;
    match crate::auth::oidc::verify_id_token(id_token, &keys, expect) {
        Err(VerifyError::UnknownKey(_)) => {
            let keys = oidc.jwks.refresh_for_unknown_key().await?;
            crate::auth::oidc::verify_id_token(id_token, &keys, expect)
        }
        other => other,
    }
}

/// Redeem the authorization code for an `id_token`.
///
/// Written here rather than through `flux_credentials::oauth_token_grant`, and the reason is
/// specific: that function returns an `OAuthToken` which **drops the raw `id_token`**, keeping only
/// an OpenAI-specific account id read out of it with an *unverified* base64 decode. The raw token
/// is precisely the artefact this story must verify, so the four lines of form encoding are written
/// out. The PKCE half — which is where a fourth implementation would actually be a hazard — is
/// still `flux-credentials`'.
async fn exchange(oidc: &Oidc, code: &str, verifier: &str) -> Result<String, String> {
    let settings: &Settings = &oidc.settings;
    // The shared back-channel client: timeouts, a connect timeout, and no redirect following. A
    // redirect here would carry the `client_secret` in the re-sent body to wherever it pointed.
    let response = oidc
        .http
        .post(&settings.token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", settings.redirect_uri.as_str()),
            ("client_id", settings.client_id.as_str()),
            ("client_secret", settings.form_secret()),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|error| format!("the token exchange did not complete: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        // **The body is deliberately not quoted.** A token endpoint that echoes the request back
        // in an error — several do — would put this host's `client_secret` into a response body
        // that `tests/host.rs` asserts is clean. The status is the diagnostic; the secret is not.
        return Err(format!("the token endpoint answered {status}"));
    }

    let body = crate::auth::oidc::read_bounded(response, crate::auth::oidc::MAX_TOKEN_BYTES)
        .await
        .map_err(|why| format!("the token endpoint's answer could not be read: {why}"))?;
    let body: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("the token endpoint's answer was not JSON: {error}"))?;
    body.get("id_token")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "the token endpoint returned no id_token".to_owned())
}

/// **Sign out, server-side.**
///
/// Revokes the record and clears the cookie, in that order. Idempotent: signing out without a
/// session is a success, because reporting "you were not signed in" tells an unauthenticated
/// caller something about a token they presented.
pub async fn signout(State(app): State<App>, request: axum::extract::Request) -> Response {
    let (parts, _) = request.into_parts();
    if let Some(token) = token_of(&parts) {
        app.sessions().revoke(&token);
    }
    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/".to_owned()),
            (header::SET_COOKIE, cleared_cookie()),
        ],
    )
        .into_response()
}

/// **Sign in as the developer — the door that needs no Google registration** (C-234).
///
/// # This route does not exist unless the process was started with `--dev`
///
/// It is added to the table in [`crate::router`] behind `App::dev_signin()`, so a host started
/// without the flag answers `404` here — not `403`. That distinction is the whole design. A route
/// that exists and refuses is one edited condition away from a route that accepts, and the edit
/// would read like a refactor; a route the router was never given cannot be reached by any
/// misconfiguration, at any log level, from any origin. Nothing in this function checks whether dev
/// mode is on, because by the time it runs the question has already been answered structurally.
///
/// # It mints an ordinary session, through the ordinary machinery
///
/// `Accounts::of_subject` → `Sessions::create` → [`session_cookie`] — the same three calls, in the
/// same order, as [`callback`]'s tail. There is no second session type, no flag on the record, and
/// no branch anywhere downstream: `Principal`, the tenant resolution, the TTL, the opacity and the
/// server-side revocation are all the same code. A dev mode that special-cased any of those would
/// make every other route behave differently under test than in production, which is precisely what
/// makes most dev modes cost more than they are worth.
///
/// The identity is fixed by [`Account::developer`] and takes no input, so this is not an
/// impersonation primitive: there is no parameter that would let a caller ask to be somebody. Its
/// tenant is `dev-local`, which no `id_token` can reach — see
/// [`crate::auth::session::DEV_TENANT`].
///
/// # Why it is a `POST`, and the one header it looks at
///
/// A `GET` would be reachable from a link or an `<img>`, which is exactly how C-204's login-CSRF
/// worked. A `POST` is not, but a cross-site *form* still is, and this route mints a session from a
/// request that carries no cookie — so `SameSite=Lax`, which protects the credential-writing
/// routes, does nothing for this one. `Sec-Fetch-Site: cross-site` is therefore refused.
///
/// That check is **defence in depth and nothing more**. The load-bearing controls are that the
/// route is absent without `--dev` and that this host binds loopback only. `Sec-Fetch-Site` is sent
/// by every current browser and by no command-line client, so an absent header is read as "not a
/// browser" and allowed — which is what keeps `curl http://localhost:8787/auth/dev` working, and
/// that is the flow the story asks to be verified by hand.
///
/// **Nothing here touches C-204's `connectors_login` binding.** This is a door beside that one. The
/// binding cookie, its constant-time comparison and the single-use `state` are untouched, and a dev
/// session is not a way to shortcut any of them — it is a different account in a different tenant.
pub async fn dev_signin(State(app): State<App>, request: axum::extract::Request) -> Response {
    let (parts, _) = request.into_parts();

    if parts
        .headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|site| site.eq_ignore_ascii_case("cross-site"))
    {
        return Failure::new(
            StatusCode::FORBIDDEN,
            "the dev sign-in is not reachable from another origin".to_owned(),
        )
        .into_response();
    }

    let account = app.accounts().of_subject(Account::developer());
    let token = app.sessions().create(account);

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/".to_owned()),
            (header::SET_COOKIE, session_cookie(&token)),
        ],
    )
        .into_response()
}

/// Who is signed in.
///
/// Carries the labels and the tenant, which is not a secret — it is the prefix of every credential
/// address this account already sees in the connector view. It carries no session token, because a
/// route that echoed one would turn any response-reflection bug into session theft.
pub async fn me(principal: Principal) -> Json<serde_json::Value> {
    let account = principal.account();
    Json(json!({
        "subject": account.subject(),
        "tenant": account.tenant(),
        "email": account.email,
        "name": account.name,
    }))
}

/// Whether sign-in is usable, and who is signed in if so.
///
/// Reachable without a session on purpose: it is what lets the page render "set these two
/// environment variables" rather than a sign-in button that leads to a `503`.
pub async fn status(
    State(app): State<App>,
    request: axum::extract::Request,
) -> Json<serde_json::Value> {
    let (parts, _) = request.into_parts();
    let signed_in = token_of(&parts).and_then(|token| app.sessions().resolve(&token));

    Json(json!({
        "configured": app.oidc().is_some(),
        "setup": app.setup_message(),
        // Whether the dev door exists on this process (C-234), so the page draws the button only
        // where it would work rather than offering one that 404s. This is the *same* value the
        // route table was built from, so the page cannot disagree with the router.
        "dev": app.dev_signin(),
        "signed_in": signed_in.is_some(),
        "account": signed_in.map(|account| json!({
            "subject": account.subject(),
            "tenant": account.tenant(),
            "email": account.email,
            "name": account.name,
        })),
    }))
}

/// Sign-in is not set up. Say what is missing, in the response an operator is looking at.
fn not_configured(app: &App) -> Response {
    Failure::new(
        StatusCode::SERVICE_UNAVAILABLE,
        app.setup_message().unwrap_or_else(|| Setup::explain(&[])),
    )
    .into_response()
}

/// A refusal during sign-in.
///
/// `400` rather than `401`: the request is malformed or unsolicited, not unauthenticated. None of
/// these messages carries a token, a code, a verifier or the client secret — they name the check
/// that failed and nothing else, which is what makes them safe to show an operator.
fn refuse(why: String) -> Response {
    Failure::new(StatusCode::BAD_REQUEST, why).into_response()
}

/// A refusal that also drops the sign-in binding.
///
/// Used for every refusal from the binding check onwards. The flow is over either way, and a
/// binding value left in the browser after its `state` has been spent is a value with nothing left
/// to bind — so it is cleared rather than left to age out.
fn refuse_and_clear(why: String) -> Response {
    let mut response = refuse(why);
    append_cookie(&mut response, crate::auth::cleared_login_cookie());
    response
}
