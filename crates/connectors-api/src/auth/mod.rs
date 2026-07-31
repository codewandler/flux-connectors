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

/// The short-lived cookie that binds a sign-in in progress to the browser that began it.
pub const LOGIN_COOKIE: &str = "connectors_login";

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

/// **The `Set-Cookie` that binds a sign-in in progress to the browser that began it.**
///
/// # Why this exists, and what its absence cost
///
/// Without it the OAuth `state` is a value held only in a process-global map, redeemable by
/// *anyone* who presents it. An attacker begins a sign-in in their own browser, keeps the `state`,
/// and gets the victim's browser to fetch the callback URL — a top-level `GET`, so a link or an
/// `<img>` is enough. The victim is then silently signed in **as the attacker**, and every
/// credential they paste lands in the attacker's tenant. That is login CSRF, and the first version
/// of this module had it: `/auth/signin` set no cookie, and `/auth/callback` never asked which
/// browser was in front of it.
///
/// Single-use consumption of the `state` does **not** close this. That is a *replay* defence — it
/// stops the same callback being redeemed twice — and the two were conflated. RFC 6749 §10.12 is
/// explicit that the binding value must be kept *"in a location accessible only to the client and
/// the user-agent"*, which means a cookie. Both properties are needed and both are now enforced.
///
/// The attributes:
///
/// - **`SameSite=Lax`, emphatically not `Strict`.** The callback arrives as a cross-site top-level
///   `GET` redirected from the identity provider. `Strict` withholds cookies on exactly that
///   navigation, so it would turn this fix into a sign-in that can never complete. `Lax` sends the
///   cookie on top-level GETs and withholds it on cross-site POSTs, which is the shape needed here.
/// - **`Path=/auth/callback`** — the only route that reads it, so it is not attached to every
///   request the browser makes.
/// - **`HttpOnly` + `Secure`** — the same reasoning as [`session_cookie`]: script cannot read it,
///   and it never travels in clear.
/// - **`Max-Age`** matching [`session::LOGIN_TTL`], because a binding value for a flow that has to
///   complete in ten minutes has no business outliving it.
///
/// # What this does not cover, and why `__Host-` is not available
///
/// This is a **double-submit**, and the cookie's value *is* the `state` that travels in the URL —
/// so the comparison in [`routes::callback`] is "these two agree", with no server-side secret in
/// it. Anyone who can *set* this cookie satisfies it by construction. That needs a cookie-injection
/// foothold (a sibling subdomain writing a `Domain` cookie, or an XSS on this origin), which is
/// outside the link-or-`<img>` threat model C-204 closes and is true of most double-submit
/// implementations; [`routes::NO_SUCH_SIGN_IN`] still holds independently, so a forged binding only
/// reopens the attacker's own live flow.
///
/// **`__Host-` would close the sibling-subdomain half and is foreclosed here:** the prefix requires
/// `Path=/`, and this cookie is deliberately scoped to `Path=/auth/callback` so it is not attached
/// to every request. The two cannot both be had, narrow scoping was chosen, and widening the path
/// is the price of revisiting it. Recorded in full, for an operator, in this crate's README under
/// "What the binding does not cover".
pub fn login_cookie(state: &str) -> String {
    format!(
        "{LOGIN_COOKIE}={state}; Path=/auth/callback; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
        session::LOGIN_TTL.as_secs()
    )
}

/// The `Set-Cookie` that clears a spent sign-in binding.
///
/// `Path` must match [`login_cookie`]'s exactly or the browser keeps the original alongside the
/// deletion, and a spent binding value sits there for its full lifetime.
pub fn cleared_login_cookie() -> String {
    format!("{LOGIN_COOKIE}=; Path=/auth/callback; HttpOnly; Secure; SameSite=Lax; Max-Age=0")
}

/// One named cookie carried by a request, if any.
pub fn cookie_of(parts: &Parts, name: &str) -> Option<String> {
    let header = parts
        .headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?;
    header
        .split(';')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| key.trim() == name)
        .map(|(_, value)| value.trim().to_owned())
}

/// The session token carried by a request, if any.
pub fn token_of(parts: &Parts) -> Option<String> {
    cookie_of(parts, SESSION_COOKIE)
}

/// The sign-in binding value carried by a request, if any.
pub fn login_state_of(parts: &Parts) -> Option<String> {
    cookie_of(parts, LOGIN_COOKIE)
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
