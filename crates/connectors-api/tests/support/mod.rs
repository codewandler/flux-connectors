//! A loopback OpenID Connect provider, for tests.
//!
//! # Why a real one rather than a stub of the verifier
//!
//! Every failure mode this story is about lives on the verification side, so a test that stubbed
//! out verification would assert the stub. This provider mints its own RSA key, serves its own
//! JWKS document and signs its own `id_token`s, which makes "a token that fails **exactly one**
//! check" something a test constructs rather than describes: change one claim, leave the rest
//! correct, and the signature is still genuine.
//!
//! It signs with `aws-lc-rs` while the crate verifies with `jsonwebtoken`. That is deliberate —
//! two independent implementations either side of the assertion. Signing with the verifier's own
//! library would let a shared misunderstanding of the encoding pass as agreement.
//!
//! Nothing here is reachable from shipped code: this module lives under `tests/`, and a
//! subdirectory of `tests/` is not compiled as a test binary of its own.

#![allow(dead_code)]

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::{KeyPair as _, RsaKeyPair, RSA_PKCS1_SHA256};
use axum::extract::State;
use axum::routing::{get, post};
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::{json, Value};

/// The OAuth client id every test registers the host under.
///
/// Shaped like a Google client id so the `aud` assertions are about a realistic value, and
/// suffixed `.test` so it can never collide with a real registration.
pub const CLIENT_ID: &str = "connectors-api-tests.apps.googleusercontent.test";

/// The client secret every test configures.
///
/// Obviously fake and long enough to be worth asserting has not leaked onto a surface. Nothing
/// shaped like a real Google secret goes in this repository — push protection has blocked a release
/// here before.
pub const CLIENT_SECRET: &str = "SENTINEL-NOT-A-REAL-SECRET-google-client-secret";

/// The `kid` the provider publishes and stamps into every header it signs.
pub const KID: &str = "test-signing-key-1";

/// One form body, as received: `[("grant_type", "authorization_code"), …]`.
pub type Form = Vec<(String, String)>;

/// Every form body the token endpoint has received, in order.
pub type TokenRequests = Arc<Mutex<Vec<Form>>>;

/// A running identity provider.
pub struct Idp {
    /// Its origin, which is also its `iss`.
    pub issuer: String,
    key: Arc<RsaKeyPair>,
    /// Every form body the token endpoint received, in order.
    ///
    /// This is how a test asserts what the host *sent* — that the PKCE `code_verifier` was present
    /// and that it is the pre-image of the challenge the authorize URL carried. Without it, PKCE
    /// would be asserted by reading the code rather than by exercising it.
    pub token_requests: TokenRequests,
}

impl Idp {
    /// Start one on an ephemeral loopback port.
    pub async fn start() -> Self {
        let key = Arc::new(
            RsaKeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).expect("an RSA-2048 key pair"),
        );
        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("an ephemeral loopback port");
        let issuer = format!("http://{}", listener.local_addr().expect("a bound address"));

        let state = IdpState {
            key: Arc::clone(&key),
            issuer: issuer.clone(),
            token_requests: Arc::new(Mutex::new(Vec::new())),
        };
        let token_requests = Arc::clone(&state.token_requests);

        let router = Router::new()
            .route("/certs", get(certs))
            .route("/token", post(token))
            .with_state(state);
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        Self {
            issuer,
            key,
            token_requests,
        }
    }

    /// Where the host should fetch signing keys.
    pub fn jwks_url(&self) -> String {
        format!("{}/certs", self.issuer)
    }

    /// Where the host should exchange an authorization code.
    pub fn token_url(&self) -> String {
        format!("{}/token", self.issuer)
    }

    /// Where a browser would be sent to consent. Nothing serves this — a test reads the redirect
    /// rather than following it, because following it is the part a human does.
    pub fn authorize_url(&self) -> String {
        format!("{}/authorize", self.issuer)
    }

    /// The JWKS document this provider publishes.
    pub fn jwks(&self) -> Value {
        jwks_of(&self.key)
    }

    /// Sign `claims` into a compact JWS with the given header.
    ///
    /// The header is a parameter so a test can vary `alg` and `kid` — the two header fields an
    /// attacker controls — without the helper quietly correcting them.
    pub fn sign_with_header(&self, header: &Value, claims: &Value) -> String {
        sign(&self.key, header, claims)
    }

    /// Sign `claims` with the ordinary RS256 header this provider uses.
    pub fn sign(&self, claims: &Value) -> String {
        self.sign_with_header(&json!({ "alg": "RS256", "typ": "JWT", "kid": KID }), claims)
    }

    /// A second key that this provider does **not** publish — the "signed by someone else" case.
    pub fn foreign_key() -> Arc<RsaKeyPair> {
        Arc::new(RsaKeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).expect("an RSA-2048 key"))
    }

    /// **A genuine key-confusion forgery.**
    ///
    /// The real attack is not "RSA bytes under an `HS256` header" — that is merely a broken
    /// signature, and a verifier could refuse it for the wrong reason while still being
    /// exploitable. The attack is: take the RSA **public** key, which the provider publishes to the
    /// world, and use its modulus as an **HMAC secret**. A verifier that reads `alg` out of the
    /// token then computes `HMAC-SHA256(public_modulus, signing_input)` — a value the attacker can
    /// compute just as easily — and the signature verifies. That is a complete authentication
    /// bypass requiring no secret at all.
    ///
    /// This produces exactly that token, so the test asserting it is refused asserts the defence
    /// rather than an accident.
    pub fn forge_by_key_confusion(&self, claims: &Value) -> String {
        let components: aws_lc_rs::rsa::PublicKeyComponents<Vec<u8>> = self.key.public_key().into();
        let header = json!({ "alg": "HS256", "typ": "JWT", "kid": KID });
        let signing_input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        // The published modulus, used as the shared secret — which is the whole trick.
        let key = aws_lc_rs::hmac::Key::new(aws_lc_rs::hmac::HMAC_SHA256, &components.n);
        let tag = aws_lc_rs::hmac::sign(&key, signing_input.as_bytes());
        format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(tag.as_ref()))
    }

    /// Sign with a key whose public half is absent from this provider's JWKS.
    pub fn sign_with_foreign_key(key: &RsaKeyPair, claims: &Value) -> String {
        sign(
            key,
            &json!({ "alg": "RS256", "typ": "JWT", "kid": KID }),
            claims,
        )
    }
}

#[derive(Clone)]
struct IdpState {
    key: Arc<RsaKeyPair>,
    issuer: String,
    token_requests: TokenRequests,
}

/// The JWKS endpoint.
async fn certs(State(state): State<IdpState>) -> axum::Json<Value> {
    axum::Json(jwks_of(&state.key))
}

/// The token endpoint.
///
/// The authorization code carries what the token should claim, as `<sub>|<nonce>`. That is not how
/// a real provider works, but it is how a test says "the browser came back from consenting as this
/// person" without the provider needing to have watched the authorize request go by.
async fn token(State(state): State<IdpState>, body: String) -> axum::Json<Value> {
    let form = parse_form(&body);
    state
        .token_requests
        .lock()
        .expect("not poisoned")
        .push(form.clone());

    let code = form
        .iter()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let (subject, nonce) = code.split_once('|').unwrap_or((code.as_str(), ""));

    let now = unix_now();
    let id_token = sign(
        &state.key,
        &json!({ "alg": "RS256", "typ": "JWT", "kid": KID }),
        &json!({
            "iss": state.issuer,
            "aud": CLIENT_ID,
            "sub": subject,
            "nonce": nonce,
            "email": format!("{subject}@example.test"),
            "email_verified": true,
            "name": subject,
            "iat": now,
            "exp": now + 3600,
        }),
    );

    axum::Json(json!({
        "access_token": "SENTINEL-NOT-A-REAL-SECRET-idp-access-token",
        "id_token": id_token,
        "token_type": "Bearer",
        "expires_in": 3600,
    }))
}

/// The JWKS document for `key`, in the RSA `n`/`e` form Google publishes.
fn jwks_of(key: &RsaKeyPair) -> Value {
    let components: aws_lc_rs::rsa::PublicKeyComponents<Vec<u8>> = key.public_key().into();
    json!({
        "keys": [{
            "kty": "RSA",
            "alg": "RS256",
            "use": "sig",
            "kid": KID,
            "n": URL_SAFE_NO_PAD.encode(&components.n),
            "e": URL_SAFE_NO_PAD.encode(&components.e),
        }]
    })
}

/// `base64url(header).base64url(claims).base64url(signature)`.
///
/// Assembled by hand rather than through a JWT library so the tests own every byte they assert
/// about — a helper that normalised a header would quietly repair the very tokens meant to be
/// broken.
fn sign(key: &RsaKeyPair, header: &Value, claims: &Value) -> String {
    let signing_input = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    );
    let mut signature = vec![0u8; key.public_modulus_len()];
    key.sign(
        &RSA_PKCS1_SHA256,
        &SystemRandom::new(),
        signing_input.as_bytes(),
        &mut signature,
    )
    .expect("the signature is produced");
    format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(&signature))
}

/// Start the host pointed at `idp`, and return its base URL.
///
/// The identity provider's endpoints are configuration, which is what lets a test substitute one.
/// See `connectors_api::auth::oidc` for why that is not a weakening: whoever can set these can
/// already set the client id and secret, so the environment is the trust root either way.
pub async fn serve(idp: &Idp) -> String {
    // Held until the `App` has read the environment. See [`env_lock`].
    let guard = env_lock();

    std::env::set_var(connectors_api::auth::oidc::CLIENT_ID_ENV, CLIENT_ID);
    std::env::set_var(connectors_api::auth::oidc::CLIENT_SECRET_ENV, CLIENT_SECRET);
    std::env::set_var(
        connectors_api::auth::oidc::REDIRECT_URI_ENV,
        "http://127.0.0.1/auth/callback",
    );
    std::env::set_var("CONNECTORS_OIDC_ISSUER", &idp.issuer);
    std::env::set_var("CONNECTORS_OIDC_AUTHORIZE_URL", idp.authorize_url());
    std::env::set_var("CONNECTORS_OIDC_TOKEN_URL", idp.token_url());
    std::env::set_var("CONNECTORS_OIDC_JWKS_URL", idp.jwks_url());

    let app = connectors_api::App::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
    drop(guard);

    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .expect("an ephemeral loopback port");
    let address = listener.local_addr().expect("a bound address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, connectors_api::router(app)).await;
    });
    format!("http://{address}")
}

/// A client that reports redirects rather than following them.
///
/// Following them would hide the two things worth asserting: where sign-in sends a browser, and
/// what the callback puts in `Set-Cookie` on the way back.
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("a client")
}

/// Complete a whole sign-in as `subject` and return the session cookie's `name=value`.
///
/// Drives the real flow end to end — `/auth/signin`, the identity provider, `/auth/callback` — so
/// the session it returns is one the host minted for a verified `id_token` rather than one a test
/// invented. There is deliberately no shortcut: a Rust back door that manufactured a session would
/// be the very thing whose absence these tests assert.
pub async fn sign_in(base: &str, subject: &str) -> String {
    let browser = client();
    let begun = begin_sign_in(base, &browser).await;

    let callback = browser
        .get(format!("{base}/auth/callback"))
        // A browser sends back the cookie `/auth/signin` set. Carried explicitly rather than
        // through a `cookie_store`, because *which* cookie travels on *which* request is the
        // property under test: `tests/tenancy.rs` completes this same call without it and must be
        // refused.
        .header("cookie", &begun.login_cookie)
        .query(&[
            ("code", format!("{subject}|{}", begun.nonce)),
            ("state", begun.state),
        ])
        .send()
        .await
        .expect("the callback completes");
    assert_eq!(
        callback.status(),
        303,
        "a good callback must establish a session and send the browser on"
    );

    session_cookie(&callback).expect("the callback set a session cookie")
}

/// What one browser holds after `GET /auth/signin`.
pub struct BegunSignIn {
    pub state: String,
    pub nonce: String,
    pub challenge: String,
    /// The `name=value` of the short-lived cookie that binds this flow to this browser.
    pub login_cookie: String,
}

/// Start a sign-in in `browser` and read back everything it now holds.
///
/// Split out from [`sign_in`] so a test can begin a flow in one browser and try to finish it in
/// another — which is the login-CSRF attack, and the thing that must not work.
pub async fn begin_sign_in(base: &str, browser: &reqwest::Client) -> BegunSignIn {
    let start = browser
        .get(format!("{base}/auth/signin"))
        .send()
        .await
        .expect("the sign-in call completes");
    assert_eq!(
        start.status(),
        303,
        "GET /auth/signin must redirect a browser to the identity provider"
    );
    let authorize = start
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("a Location header")
        .to_owned();

    BegunSignIn {
        state: query_param(&authorize, "state").expect("the authorize URL carries a state"),
        nonce: query_param(&authorize, "nonce").expect("the authorize URL carries a nonce"),
        challenge: query_param(&authorize, "code_challenge").expect("a challenge"),
        login_cookie: cookie_named(&start, connectors_api::auth::LOGIN_COOKIE).unwrap_or_else(
            || {
                panic!(
                    "GET /auth/signin set no `{}` cookie, so nothing binds this flow to this \
                     browser — see tests/tenancy.rs's login-CSRF case",
                    connectors_api::auth::LOGIN_COOKIE
                )
            },
        ),
    }
}

/// The `name=value` of the session cookie this response sets, if it sets one.
pub fn session_cookie(response: &reqwest::Response) -> Option<String> {
    cookie_named(response, connectors_api::auth::SESSION_COOKIE)
}

/// The `name=value` of one named `Set-Cookie` on this response, ignoring cleared ones.
///
/// A response may carry several `Set-Cookie` headers — the successful callback sets the session and
/// clears the login cookie in one go — so this looks at all of them rather than the first. A
/// deletion (`Max-Age=0`) is not a cookie a browser would hold, so it is skipped.
pub fn cookie_named(response: &reqwest::Response, name: &str) -> Option<String> {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter(|header| !header.to_lowercase().contains("max-age=0"))
        .filter_map(|header| header.split(';').next())
        .map(str::trim)
        .find(|pair| pair.split_once('=').is_some_and(|(key, _)| key == name))
        .map(str::to_owned)
}

/// Every `Set-Cookie` header on a response, whole, for asserting attributes.
pub fn set_cookie_headers(response: &reqwest::Response) -> Vec<String> {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(str::to_owned)
        .collect()
}

/// One query parameter of a URL, decoded.
pub fn query_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| decode(value))
}

/// Held across "set the environment, then build the `App`".
///
/// The host reads its sign-in configuration from the environment **once**, at construction, which
/// is what makes a misconfiguration a startup fact. The environment is per *process* while
/// `cargo test` runs a binary's tests on parallel threads, so two tests pointing two hosts at two
/// identity providers will interleave their `set_var` calls and hand one host the other's
/// endpoints. Guarding the whole set-then-construct window is what makes each test's provider its
/// own; nothing after construction reads the environment again.
pub fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Seconds since the Unix epoch.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_secs() as i64
}

/// `a=1&b=2` into pairs, percent-decoding both halves.
///
/// Hand-written rather than reached for through axum's `Form` extractor so the dev-dependency does
/// not need the `form` feature for four lines of splitting.
fn parse_form(body: &str) -> Vec<(String, String)> {
    body.split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| {
            (
                decode(&key.replace('+', " ")),
                decode(&value.replace('+', " ")),
            )
        })
        .collect()
}

fn decode(value: &str) -> String {
    urlencoding::decode(value)
        .map(|decoded| decoded.into_owned())
        .unwrap_or_else(|_| value.to_owned())
}
