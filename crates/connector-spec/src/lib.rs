//! The connector IR and its two front-ends.
//!
//! A connector is described once — either as a pointer at a vendor OpenAPI document plus patches, or
//! as a complete hand-authored definition — and normalized into one [`Connector`] value that
//! `connector-flux` compiles into a Flux-Lang module.
//!
//! **This crate performs no network IO.** Ingest takes bytes, so every stage here is a pure,
//! unit-testable function; fetching lives in `connector-cli` alone.
//!
//! # The shape
//!
//! ```text
//! Connector { id, vendor, base_url, auth: [AuthMethod], default_auth: [AuthRequirement],
//!             operations: [Operation { id, method, path, params, response_schema,
//!                                      risk, idempotency, description,
//!                                      auth: Option<[AuthRequirement]>, quirks }],
//!             provenance }
//! ```
//!
//! Two things about it are worth reading the docs on before using it:
//!
//! - **Auth is many-to-many.** A connector declares several [`AuthMethod`]s; each [`Operation`]
//!   selects among them with a list of [`AuthRequirement`]s — AND within a requirement, OR across
//!   the list — and distinguishes *unset* (inherit the connector default) from *explicitly none*.
//!   See [`Operation::auth`] and [`Connector::effective_auth`].
//! - **The encoding is canonical.** Equal values produce identical bytes, because
//!   [`Connector::canonical_json`] is what `connectors.lock` hashes.

mod auth;
mod ir;

pub use auth::{AuthMethod, AuthRequirement, AuthScheme, OAuth2Spec, OAuthGrant, OAuthRedirect};
pub use ir::{
    Connector, ErrorEnvelope, HttpMethod, Idempotency, JsonSchema, Operation, Pagination, Param,
    ParamSet, Provenance, Quirks, RateLimit, Risk,
};

/// Everything that can go wrong turning a provider definition into a [`Connector`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A provider definition could not be parsed or validated.
    #[error("invalid provider definition: {0}")]
    Invalid(String),
    /// The IR could not be encoded — see [`Connector::canonical_json`].
    #[error("cannot serialize the connector IR: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// The crate's result alias.
pub type Result<T> = std::result::Result<T, Error>;
