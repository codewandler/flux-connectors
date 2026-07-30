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
//! - **The encoding is canonical.** Equal values produce identical bytes — the property
//!   [`Connector::hash_domain`] and therefore `connectors.lock` rest on.
//! - **The types are strict.** Every one of them denies unknown fields, because the provider loader
//!   parses `providers/*.toml` straight into them and a typo'd key must not be discarded before the
//!   loader can object. `src/ir.rs`'s module docs record the two failures C-2's review
//!   demonstrated, and `tests/strict_fields.rs` pins them.
//!
//! # Loading a provider file
//!
//! [`provider::load`] is the entry point: bytes in, a validated [`LoadedProvider`] out. It serves
//! both roles the file plays — a complete hand-authored connector, or a pointer at a vendored spec
//! plus the patch set the overlay applies. `schema/provider-toml.schema.json`
//! ([`PROVIDER_TOML_JSON_SCHEMA`]) documents the file format and is kept in sync by a test.
//!
//! # Recording what produced an artifact
//!
//! [`Lockfile`] renders `connectors.lock`: one [`LockEntry`] per provider, holding the hashes and
//! versions `flux-connectors check` recomputes. Its contract is that unchanged inputs reproduce the
//! file byte for byte — see the [`lock`] module docs for what is hashed, what is deliberately not,
//! and why no timestamp appears anywhere in it.

mod auth;
mod ir;
pub mod lock;
pub mod provider;

pub use auth::{AuthMethod, AuthRequirement, AuthScheme, OAuth2Spec, OAuthGrant, OAuthRedirect};
pub use ir::{
    Connector, ErrorEnvelope, HttpMethod, Idempotency, JsonSchema, Operation, Pagination, Param,
    ParamSet, Provenance, Quirks, RateLimit, Risk,
};
pub use lock::{sha256_hex, LockEntry, Lockfile, LOCKFILE_NAME, LOCKFILE_VERSION};
pub use provider::{
    LoadedProvider, OperationPatch, ParamPatch, ParamPosition, Patch, SpecSource,
    PROVIDER_TOML_JSON_SCHEMA,
};

/// Everything that can go wrong turning a provider definition into a [`Connector`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A provider definition could not be parsed or validated.
    #[error("invalid provider definition: {0}")]
    Invalid(String),
    /// A `providers/<name>.toml` is not well-formed TOML, or does not match the provider schema.
    ///
    /// The inner error is `toml`'s, kept verbatim and boxed: its `Display` carries the line, the
    /// column, a snippet of the offending source, and — for an unknown key — the list of keys that
    /// *would* have been valid. Rewriting any of that in our own words would make the message worse.
    #[error("{name}: {source}")]
    ParseProvider {
        /// The file the error is about, e.g. `providers/zendesk.toml`.
        name: String,
        /// `toml`'s own parse/deserialize error. Boxed because it is large and this enum is
        /// returned by value from every loader entry point.
        #[source]
        source: Box<toml::de::Error>,
    },
    /// A `providers/<name>.toml` parsed, but is not a valid connector definition.
    ///
    /// Carries **every** problem found rather than the first. Fixing a provider file one error per
    /// run is the authoring experience this crate exists to avoid.
    #[error("{}", render_problems(.name, .problems))]
    InvalidProvider {
        /// The file the errors are about.
        name: String,
        /// One line per problem, each naming the operation or credential it is about.
        problems: Vec<String>,
    },
    /// The IR could not be encoded — see [`Connector::canonical_json`].
    #[error("cannot serialize the connector IR: {0}")]
    Serialize(#[from] serde_json::Error),
    /// `connectors.lock` could not be rendered — see [`Lockfile::to_toml`].
    #[error("cannot serialize connectors.lock: {0}")]
    SerializeLock(#[from] Box<toml::ser::Error>),
    /// A `connectors.lock` is not well-formed — see [`Lockfile::parse`].
    ///
    /// As with [`ParseProvider`](Self::ParseProvider), `toml`'s message is kept verbatim: it names
    /// the offending key with a line, a column and a snippet, which no rewording improves on. Boxed
    /// for the same reason, too: the error is large and this enum is returned by value everywhere.
    #[error("connectors.lock is not a valid lockfile: {0}")]
    ParseLock(#[from] Box<toml::de::Error>),
}

/// Renders [`Error::InvalidProvider`] as a heading plus one bullet per problem.
///
/// A list, not a comma-joined sentence: an author scanning for the operation they just edited finds
/// it on its own line, and the rendering is stable enough to pin with a golden file.
fn render_problems(name: &str, problems: &[String]) -> String {
    let mut rendered = format!("{name} is not a valid connector definition:");
    for problem in problems {
        rendered.push_str("\n  - ");
        rendered.push_str(problem);
    }
    rendered
}

/// The crate's result alias.
pub type Result<T> = std::result::Result<T, Error>;
