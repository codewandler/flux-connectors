//! Google OpenID Connect: what the host asks for, and what it will believe.
//!
//! # The one thing this module exists to do
//!
//! An `id_token` arriving at the callback is **attacker-controlled bytes** until every one of five
//! checks passes. Each closes a distinct attack, and skipping any one of them leaves an
//! authentication bypass that looks exactly like a working sign-in:
//!
//! | check | what it stops |
//! |---|---|
//! | signature, against Google's published JWKS | a token minted by anyone at all |
//! | `iss` | a token from a different, attacker-run OIDC provider |
//! | `aud` | a token Google *did* issue, to some **other** application — the classic OAuth confused deputy |
//! | `exp` | a token that was valid once, replayed later |
//! | `nonce` | a token obtained in some other session and replayed into this one |
//!
//! `alg` is pinned to `RS256` rather than read from the token, because the header is the
//! attacker's too: `alg: none` and HMAC-with-the-public-key are both alg-confusion attacks that a
//! verifier trusting the header performs on request.
//!
//! # Why a JWT library rather than hand-rolled parsing
//!
//! C-203's manifest recorded "no JWT library" as deliberate, on the reasoning that identity would
//! be read from the userinfo endpoint over TLS. C-204 reverses that: its acceptance requires the
//! signature checked, and TLS to Google proves who answered, not who *signed*. Given that a
//! signature must be checked, hand-rolling the parse is the worse trade — `flux-credentials`' own
//! `jwt_payload` is an unverified base64 decode, correct for reading a non-security-critical label
//! and useless as an authentication decision.
//!
//! # Why the endpoints are configurable
//!
//! [`Settings::from_env`] reads the issuer and the three endpoint URLs from the environment,
//! defaulting to Google's. That is not a weakening. Whoever can set `CONNECTORS_OIDC_ISSUER` can
//! already set `CONNECTORS_GOOGLE_CLIENT_ID` and `CONNECTORS_GOOGLE_CLIENT_SECRET`, so the
//! environment is this host's trust root either way and no privilege is crossed. What it buys is
//! a test that drives the real flow against a provider it controls, instead of a stub of the code
//! under test.

use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

/// The environment variable naming the OAuth client id.
pub const CLIENT_ID_ENV: &str = "CONNECTORS_GOOGLE_CLIENT_ID";
/// The environment variable naming the OAuth client secret.
pub const CLIENT_SECRET_ENV: &str = "CONNECTORS_GOOGLE_CLIENT_SECRET";
/// The environment variable naming the redirect URI registered with Google.
pub const REDIRECT_URI_ENV: &str = "CONNECTORS_GOOGLE_REDIRECT_URI";

/// Google's issuer, as it appears in an `id_token`.
///
/// Two spellings are accepted because Google itself uses both, and a host accepting only one
/// rejects real tokens.
const GOOGLE_ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];
const GOOGLE_AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";

/// The scopes sign-in asks for.
///
/// `openid email profile` and nothing else. This proves **who the operator is**; it mints no token
/// for `google-gmail-message-get`, which is C-207's flow with different scopes and a different
/// consent screen. Widening this list here would ask an operator to grant vendor access at the
/// door, before they have chosen to connect anything.
pub const SCOPES: &str = "openid email profile";

/// How long a fetched JWKS is reused before it is fetched again.
const JWKS_TTL: Duration = Duration::from_secs(3600);
/// The shortest interval between two forced refetches, so an unknown `kid` cannot be used to drive
/// unbounded requests at Google.
const JWKS_MIN_REFETCH: Duration = Duration::from_secs(60);
/// Clock skew tolerated on `exp`.
const LEEWAY: u64 = 60;

/// What an operator must supply, and what it defaults to.
#[derive(Clone)]
pub struct Settings {
    /// The OAuth client id. Public by design — it appears in every authorize URL.
    pub client_id: String,
    /// The OAuth client secret.
    ///
    /// Private, with no getter. The only thing that reads it is the token exchange in
    /// [`Settings::form_secret`], which hands it straight to `reqwest`. A `Debug` impl is written
    /// by hand below so that no diagnostic can print it by accident.
    client_secret: String,
    /// The callback this host is registered at, which Google checks against its own record.
    pub redirect_uri: String,
    /// Every `iss` value an `id_token` may carry.
    pub issuers: Vec<String>,
    pub authorize_url: String,
    pub token_url: String,
    pub jwks_url: String,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("redirect_uri", &self.redirect_uri)
            .field("issuers", &self.issuers)
            .finish_non_exhaustive()
    }
}

impl Settings {
    /// The client secret, for the one caller entitled to it.
    ///
    /// Deliberately not a `pub fn secret()`. It is `pub(crate)` and named for its single use, so
    /// that a route which wanted to echo configuration back has nothing convenient to reach for.
    pub(crate) fn form_secret(&self) -> &str {
        &self.client_secret
    }

    /// Read the sign-in configuration from the environment.
    ///
    /// **The secret is resolved from the environment and never from a file in this repository** —
    /// not a provider TOML, not a generated artifact, not a checked-in config. That is an
    /// acceptance item of C-204 and also the rule `AGENTS.md` already states for every other
    /// credential in the tree.
    pub fn from_env() -> Setup {
        let client_id = non_empty(CLIENT_ID_ENV);
        let client_secret = non_empty(CLIENT_SECRET_ENV);

        let mut missing = Vec::new();
        if client_id.is_none() {
            missing.push(CLIENT_ID_ENV);
        }
        if client_secret.is_none() {
            missing.push(CLIENT_SECRET_ENV);
        }
        if !missing.is_empty() {
            return Setup::Missing(missing);
        }

        let issuers = match non_empty("CONNECTORS_OIDC_ISSUER") {
            Some(issuer) => vec![issuer],
            None => GOOGLE_ISSUERS.iter().map(|s| (*s).to_owned()).collect(),
        };

        Setup::Configured(Self {
            client_id: client_id.expect("checked above"),
            client_secret: client_secret.expect("checked above"),
            redirect_uri: non_empty(REDIRECT_URI_ENV)
                .unwrap_or_else(|| "http://localhost:8787/auth/callback".to_owned()),
            issuers,
            authorize_url: non_empty("CONNECTORS_OIDC_AUTHORIZE_URL")
                .unwrap_or_else(|| GOOGLE_AUTHORIZE_URL.to_owned()),
            token_url: non_empty("CONNECTORS_OIDC_TOKEN_URL")
                .unwrap_or_else(|| GOOGLE_TOKEN_URL.to_owned()),
            jwks_url: non_empty("CONNECTORS_OIDC_JWKS_URL")
                .unwrap_or_else(|| GOOGLE_JWKS_URL.to_owned()),
        })
    }
}

fn non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Whether sign-in is configured, and what is missing if it is not.
///
/// A host with no Google registration still starts, still serves its page, and says exactly which
/// variables are unset. The alternative — panicking at startup — turns a first run into a stack
/// trace, and the alternative to *that* — starting and failing at the first click — is the broken
/// page this shape exists to avoid.
pub enum Setup {
    Configured(Settings),
    Missing(Vec<&'static str>),
}

impl Setup {
    /// The operator-facing explanation of what is not set up yet.
    pub fn explain(missing: &[&'static str]) -> String {
        format!(
            "Google sign-in is not configured: {} {} unset.\n\
             \n\
             Register an OAuth 2.0 Client ID of type \"Web application\" at\n\
             https://console.cloud.google.com/apis/credentials, add\n\
             `http://localhost:8787/auth/callback` as an Authorized redirect URI, then set:\n\
             \n\
               {CLIENT_ID_ENV}\n\
               {CLIENT_SECRET_ENV}\n\
               {REDIRECT_URI_ENV}   (optional; defaults to the URI above)\n",
            missing.join(", "),
            if missing.len() == 1 { "is" } else { "are" },
        )
    }
}

/// A signing-key document, as a JWKS endpoint serves it.
///
/// Wraps `jsonwebtoken`'s type rather than re-exporting it, so the crate's public surface is the
/// JSON a JWKS endpoint actually returns and a caller — including a test — never has to name the
/// JWT library.
pub struct Jwks(JwkSet);

impl Jwks {
    /// Parse a JWKS document.
    ///
    /// # Errors
    ///
    /// [`VerifyError::Jwks`] if the document is not a well-formed key set.
    pub fn from_json(document: &str) -> Result<Self, VerifyError> {
        serde_json::from_str(document)
            .map(Jwks)
            .map_err(|error| VerifyError::Jwks(error.to_string()))
    }
}

/// What an `id_token` must agree with to be believed.
pub struct Expectations<'a> {
    /// Every acceptable `iss`.
    pub issuers: &'a [String],
    /// This host's own OAuth client id.
    pub audience: &'a str,
    /// The `nonce` this host put in the authorize URL that started **this** sign-in.
    pub nonce: &'a str,
}

/// The claims this host reads, and no others.
#[derive(Debug, Deserialize)]
pub struct IdClaims {
    /// The stable subject identifier. **This is the account key.**
    pub sub: String,
    pub nonce: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
}

/// Why an `id_token` was not believed.
///
/// One variant per check, deliberately: a single `Invalid` would make "this token fails exactly
/// one of them" untestable, which is the assertion C-204's acceptance asks for by name.
#[derive(Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// Not a JWT, or not one this host will consider — including a header naming any `alg` other
    /// than `RS256`.
    Malformed(String),
    /// The header named no key, or named one the provider does not publish.
    UnknownKey(Option<String>),
    /// The signature is not the provider's.
    Signature,
    /// `iss` is not one this host accepts.
    Issuer,
    /// `aud` is some other application's client id.
    Audience,
    /// `exp` is in the past.
    Expired,
    /// `nonce` is absent, or is not the one this sign-in asked for.
    Nonce,
    /// `sub` is missing, or is not usable as a tenant path segment.
    Subject(String),
    /// The key set could not be read or fetched.
    Jwks(String),
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(why) => write!(f, "the id_token is malformed: {why}"),
            Self::UnknownKey(kid) => match kid {
                Some(kid) => write!(
                    f,
                    "the id_token names signing key `{kid}`, which the provider does not publish"
                ),
                None => write!(f, "the id_token names no signing key"),
            },
            Self::Signature => write!(f, "the id_token's signature is not the provider's"),
            Self::Issuer => write!(f, "the id_token was issued by someone else"),
            Self::Audience => write!(f, "the id_token was issued for a different application"),
            Self::Expired => write!(f, "the id_token has expired"),
            Self::Nonce => write!(f, "the id_token's nonce does not match this sign-in"),
            Self::Subject(why) => write!(f, "the id_token's subject is unusable: {why}"),
            Self::Jwks(why) => write!(f, "the provider's signing keys could not be read: {why}"),
        }
    }
}

impl std::error::Error for VerifyError {}

/// **Verify an `id_token`, or refuse it.**
///
/// Every check in the table at the top of this module runs, and the first that fails names itself.
/// The order matters for the tests but not for safety: nothing is read out of the token until all
/// of them have passed, and the returned [`IdClaims`] is the only thing a caller sees.
///
/// # Errors
///
/// A [`VerifyError`] naming the single check that failed.
pub fn verify_id_token(
    token: &str,
    jwks: &Jwks,
    expect: &Expectations<'_>,
) -> Result<IdClaims, VerifyError> {
    let header = decode_header(token).map_err(|error| VerifyError::Malformed(error.to_string()))?;

    // Pinned, never read from the token. A verifier that honours the header's `alg` can be told
    // `none`, or told to treat the RSA public key as an HMAC secret — the two classic alg-confusion
    // bypasses. `Algorithm` has no `none` variant and this comparison admits one value.
    if header.alg != Algorithm::RS256 {
        return Err(VerifyError::Malformed(format!(
            "expected alg RS256, the token says {:?}",
            header.alg
        )));
    }

    let kid = header.kid.ok_or(VerifyError::UnknownKey(None))?;
    let jwk = jwks
        .0
        .find(&kid)
        .ok_or_else(|| VerifyError::UnknownKey(Some(kid.clone())))?;
    // Google publishes RSA keys. Anything else is a document this host does not understand, and
    // guessing at it is how a verifier ends up accepting a key type it cannot really check.
    if !matches!(jwk.algorithm, AlgorithmParameters::RSA(_)) {
        return Err(VerifyError::UnknownKey(Some(kid)));
    }
    let key = DecodingKey::from_jwk(jwk).map_err(|error| VerifyError::Jwks(error.to_string()))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(expect.issuers);
    validation.set_audience(&[expect.audience]);
    validation.validate_exp = true;
    validation.leeway = LEEWAY;
    // `set_issuer`/`set_audience` add `iss`/`aud` to the required set, so a token that simply omits
    // one is refused rather than silently passing the check it left out. `sub` is required because
    // it is the account key and a token without one has nothing to key on.
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);

    let data = decode::<IdClaims>(token, &key, &validation).map_err(|error| {
        use jsonwebtoken::errors::ErrorKind;
        match error.kind() {
            ErrorKind::InvalidSignature => VerifyError::Signature,
            ErrorKind::InvalidIssuer => VerifyError::Issuer,
            ErrorKind::InvalidAudience => VerifyError::Audience,
            ErrorKind::ExpiredSignature => VerifyError::Expired,
            other => VerifyError::Malformed(format!("{other:?}")),
        }
    })?;

    // The nonce is checked here rather than by the JWT library, which has no notion of it. Compared
    // without an early exit: a nonce is a per-sign-in secret, and a comparison that stops at the
    // first differing byte answers "how much of it did you guess" to anyone who can time it.
    let presented = data.claims.nonce.as_deref().unwrap_or_default();
    if !constant_time_eq(presented.as_bytes(), expect.nonce.as_bytes()) {
        return Err(VerifyError::Nonce);
    }

    Ok(data.claims)
}

/// Byte equality that does not return early.
///
/// The length comparison *is* an early exit, and that is deliberate and safe: the length of a nonce
/// this host generated is a constant, not a secret.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

/// The provider's signing keys, fetched and kept for a while.
///
/// Google rotates these, so a host that fetched once and cached forever stops being able to verify
/// anything a day later; one that fetches per sign-in makes Google a hard dependency of every
/// click. This does neither: a document is reused for [`JWKS_TTL`], and an unknown `kid` — which is
/// what a rotation looks like from here — forces one early refetch, rate-limited by
/// [`JWKS_MIN_REFETCH`] so that a stream of bogus `kid`s cannot turn this host into a load
/// generator pointed at Google.
pub struct JwksCache {
    url: String,
    http: reqwest::Client,
    state: RwLock<Option<Cached>>,
}

struct Cached {
    keys: Arc<Jwks>,
    fetched_at: Instant,
}

impl JwksCache {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            http: reqwest::Client::new(),
            state: RwLock::new(None),
        }
    }

    /// The current key set, fetching if there is none or it has aged out.
    ///
    /// # Errors
    ///
    /// [`VerifyError::Jwks`] if the document cannot be fetched or parsed.
    pub async fn current(&self) -> Result<Arc<Jwks>, VerifyError> {
        if let Some(cached) = self.state.read().await.as_ref() {
            if cached.fetched_at.elapsed() < JWKS_TTL {
                return Ok(Arc::clone(&cached.keys));
            }
        }
        self.refetch().await
    }

    /// Fetch again, unless one just happened.
    ///
    /// Returns the cached document rather than an error when the rate limit applies, so the caller
    /// reports "this key is unknown" rather than "the provider is unreachable" — two very
    /// different things to tell an operator.
    pub async fn refresh_for_unknown_key(&self) -> Result<Arc<Jwks>, VerifyError> {
        if let Some(cached) = self.state.read().await.as_ref() {
            if cached.fetched_at.elapsed() < JWKS_MIN_REFETCH {
                return Ok(Arc::clone(&cached.keys));
            }
        }
        self.refetch().await
    }

    async fn refetch(&self) -> Result<Arc<Jwks>, VerifyError> {
        let response = self
            .http
            .get(&self.url)
            .send()
            .await
            .map_err(|error| VerifyError::Jwks(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(VerifyError::Jwks(format!("{} answered {status}", self.url)));
        }
        let body = response
            .text()
            .await
            .map_err(|error| VerifyError::Jwks(error.to_string()))?;
        let keys = Arc::new(Jwks::from_json(&body)?);

        *self.state.write().await = Some(Cached {
            keys: Arc::clone(&keys),
            fetched_at: Instant::now(),
        });
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nonce_comparison_is_length_aware_and_correct() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }

    /// An absent `nonce` must not compare equal to an expected one. The natural bug is
    /// `unwrap_or_default()` against an expectation that is also empty.
    #[test]
    fn an_empty_presented_nonce_never_matches_a_real_one() {
        assert!(!constant_time_eq(b"", b"a-real-nonce"));
    }

    #[test]
    fn a_missing_client_id_is_reported_by_name() {
        let message = Setup::explain(&[CLIENT_ID_ENV]);
        assert!(message.contains(CLIENT_ID_ENV));
        assert!(message.contains("console.cloud.google.com"));
    }
}
