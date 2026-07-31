//! **Sign-in: who is asking.**
//!
//! Until this module existed the host answered every request as one hardcoded tenant, which is a
//! service holding credentials for nobody in particular.
//! [`connectors-proxy.md`](../../../../docs/designs/connectors-proxy.md) rejected exactly that
//! shape — *"a credential-injecting proxy is, by construction, a confused-deputy machine: its
//! entire job is to add authority a caller does not have"* — and dismissed "the service is
//! authenticated" as an insufficient answer. The sufficient one is **the caller is the principal
//! whose credential is being used**, and that equation is what this module establishes.
//!
//! # The shape of it
//!
//! | | where it lives |
//! |---|---|
//! | what a person proves to Google | [`oidc`] — authorization code + PKCE, and the five checks on the `id_token` |
//! | who they turn out to be | [`session::Account`] — keyed on the OIDC `sub` |
//! | how the next request knows | [`session::Sessions`] — an opaque server-side token in an `HttpOnly` cookie |
//! | what a handler receives | [`Principal`] — an extractor, so a handler cannot forget to ask |
//!
//! # Why the tenant is an extractor and not a parameter
//!
//! [`Principal`] is an axum extractor, so a handler that needs a tenant **names it in its
//! signature** and one that does not cannot reach it. The alternative — middleware that stuffs a
//! tenant into request extensions, read with an `unwrap` in each handler — fails open: a handler
//! added later that forgets the read gets no compile error, and a handler that reads the wrong
//! key gets a panic in production rather than a refusal. Here the type system asks the question.

pub mod oidc;
pub mod routes;
pub mod session;

use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;

use crate::api::Failure;
use crate::state::App;
use session::{Account, SESSION_TTL};

/// The cookie the session token travels in.
///
/// Not `__Host-`-prefixed, which would additionally pin it to this exact origin with no `Domain`.
/// That prefix requires `Secure` to be honoured by the browser, and the failure mode when it is
/// not is a sign-in that silently never completes — a bad trade for a host whose first job is to
/// be runnable locally. Recorded rather than overlooked.
pub const SESSION_COOKIE: &str = "connectors_session";

/// The `Set-Cookie` that establishes a session.
///
/// Every attribute is load-bearing:
///
/// - **`HttpOnly`** — script cannot read it, so an XSS anywhere on this origin does not become a
///   stolen session.
/// - **`Secure`** — it never travels over plaintext. Browsers treat `http://localhost` as a
///   trustworthy origin and accept `Secure` cookies there, so this costs nothing locally and is
///   the whole game the moment this host is deployed anywhere else.
/// - **`SameSite=Lax`** — a cross-site POST carries no session, which is what stops a third-party
///   page from driving this host's credential-writing routes as the operator.
/// - **`Max-Age`** — the browser forgets it, matching the server-side expiry rather than trusting
///   it alone.
///
/// The value is opaque: 32 bytes of OS entropy naming a server-side record. **No credential
/// material, no tenant, and no claim about the account is in it.**
pub fn session_cookie(token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
        SESSION_TTL.as_secs()
    )
}

/// The `Set-Cookie` that clears one.
///
/// Paired with — never a substitute for — [`session::Sessions::revoke`]. Clearing the cookie ends
/// the session for the browser that asked; revoking ends it for every copy.
pub fn cleared_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0")
}

/// The session token carried by a request, if any.
pub fn token_of(parts: &Parts) -> Option<String> {
    let header = parts
        .headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?;
    header
        .split(';')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| name.trim() == SESSION_COOKIE)
        .map(|(_, value)| value.trim().to_owned())
}

/// **Whose data this request is about.**
///
/// The tenant comes from here and from nowhere else. There is deliberately no constructor taking a
/// tenant, no `From<&str>`, and no way to build one out of a path segment, a body field or a
/// header: the only way to obtain a `Principal` is to present a live session cookie, which is what
/// makes `crates/connectors-api/tests/tenancy.rs`'s assertion structural rather than a convention
/// every handler has to remember.
pub struct Principal(Arc<Account>);

impl Principal {
    /// The tenant every port is bound for.
    pub fn tenant(&self) -> &str {
        self.0.tenant()
    }

    /// The account itself, for the one route that reports who is signed in.
    pub fn account(&self) -> &Account {
        &self.0
    }
}

impl FromRequestParts<App> for Principal {
    type Rejection = Failure;

    async fn from_request_parts(parts: &mut Parts, app: &App) -> Result<Self, Self::Rejection> {
        let token = token_of(parts).ok_or_else(unauthenticated)?;
        // Resolving also enforces expiry, so a cookie outliving its session is refused rather than
        // honoured until something happens to prune the map.
        let account = app.sessions().resolve(&token).ok_or_else(unauthenticated)?;
        Ok(Self(account))
    }
}

/// The one refusal. It says nothing about whether the session was absent, unknown or expired —
/// three answers that would let a caller probe which tokens once existed.
fn unauthenticated() -> Failure {
    Failure::new(
        StatusCode::UNAUTHORIZED,
        "sign in first: this host resolves the tenant from the session, and there is none"
            .to_owned(),
    )
}
