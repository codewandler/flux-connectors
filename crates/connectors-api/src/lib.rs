//! **The host.** Binds the connector pack's three ports and runs its operations.
//!
//! This is the caller the rest of the repository has never had. `connector-pack` projects a
//! catalogue operation onto a flux `ToolSpec`, evaluates a request out of the operation's own
//! emitted Flux, resolves and places the credential, and delegates the send — and until this crate
//! existed, nothing constructed a `ToolRegistry` outside a test.
//!
//! # What this crate does not do
//!
//! **It constructs no request.** Every route below ends in `connector_pack::pack`, and the
//! `{ method, url, headers, body }` that reaches a vendor is the one the pack evaluated from the
//! operation's Flux. That is the property that made `connectors-app` supersede `connectors-proxy`
//! (`docs/designs/connectors-app.md`): a second implementation of request construction is the drift
//! [`C-117`] exists to catch, and a host that builds its own requests is a second opinion about what
//! an operation *is*.
//!
//! **It ships no transport of its own.** `flux_web::http::HttpRequestTool` is flux's, configured
//! once here and handed to every operation as an `Egress`, so connectors inherit the host's egress
//! allow-list and SSRF guard rather than a policy this crate invented.
//!
//! # The tenant is the session's, and only the session's
//!
//! Every port is constructed per tenant — `Credentials::new(store, tenant)` and
//! `Configuration::new(values, tenant)` — and the tenant comes from the signed-in account, never
//! from a path segment, a body field or a header. `connector_pack::Error::TenantMismatch` exists for
//! the case where a host pairs two ports wrongly; here the pair is built from one value at one call
//! site, so it is unreachable by construction.
//!
//! **That is now enforced rather than intended** (C-204). The tenant is reached only through
//! [`auth::Principal`], an extractor whose sole constructor is a live session cookie: there is no
//! `Principal::from(&str)`, no tenant path segment, and no header a caller could set. A handler
//! that needs a tenant names it in its signature and one that does not cannot reach it.
//! `crates/connectors-api/tests/tenancy.rs` drives the whole flow against a loopback identity
//! provider and asserts that a request naming tenant B while carrying tenant A's session resolves
//! to **A**.
//!
//! [`C-117`]: https://github.com/codewandler/flux-connectors/blob/main/docs/stories/C-117-pack-codegen.md

#![forbid(unsafe_code)]

pub mod api;
pub mod auth;
pub mod config;
pub mod exec;
pub mod state;
pub mod ui;

pub use state::App;

use axum::routing::{get, post, put};
use axum::Router;

/// Every route the host serves.
///
/// Split out from the binary so an integration test can drive it without binding a port, and so the
/// route table is one readable list rather than something assembled across three files.
pub fn router(app: App) -> Router {
    Router::new()
        // The UI.
        .route("/", get(ui::index))
        // Sign-in. These five are the only routes reachable without a session, and each is
        // reachable without one for a reason: two are the flow that establishes a session, one
        // ends it, one reports whether sign-in is even configured, and `/` has to render something
        // to sign in *from*.
        //
        // **Every route under `/v1` takes a `Principal`**, with no exceptions. The review that
        // followed C-204's first landing found this comment claiming as much while
        // `/v1/operations/{operation}` did not. It served only published catalogue data, so nothing
        // tenant-scoped was leaking — but "all of them except one, and that one is fine" is a rule
        // nobody can check at a glance, and the exception would have been inherited by whatever
        // was added next to it. The route is gated rather than the comment softened: the catalogue
        // is public through `web/public/catalog.json` anyway, so gating costs nothing and buys a
        // rule with no footnote.
        .route("/auth/signin", get(auth::routes::signin))
        .route("/auth/callback", get(auth::routes::callback))
        .route("/auth/signout", post(auth::routes::signout))
        .route("/auth/status", get(auth::routes::status))
        .route("/auth/me", get(auth::routes::me))
        // The catalogue — read straight from `connector-catalog`, never a hand-kept list.
        .route("/v1/connectors", get(api::connectors))
        .route("/v1/connectors/{provider}", get(api::connector))
        .route("/v1/operations/{operation}", get(api::operation))
        // What an operator supplies.
        .route(
            "/v1/credentials/{provider}/{credential}",
            put(api::put_credential).delete(api::delete_credential),
        )
        .route(
            "/v1/config/{provider}/{service}/{kind}/{field}",
            put(api::put_config),
        )
        // What it is all for.
        .route("/v1/operations/{operation}/execute", post(api::execute))
        .with_state(app)
}
