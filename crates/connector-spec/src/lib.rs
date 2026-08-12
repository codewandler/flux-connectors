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
//! A [`Connector`] is **not** a set of operations. It declares what a vendor can do in **both
//! directions**, and what an **operator** must supply to use it. All sixteen fields:
//!
//! ```text
//! Connector { id, authority, runtime, api_version, vendor, base_url, description,
//!             services: [Service { name, description, base_url, api_version, roles }],
//!             auth: [AuthMethod], default_auth: [AuthRequirement],
//!             operations: [Operation { id, service, method, path, params, response_schema,
//!                                      risk, idempotency, description,
//!                                      auth: Option<[AuthRequirement]>, quirks }],
//!             events:   [EventDecl { name, service, description, when, schema }],
//!             channels: [ChannelBinding { name, service, transport, events, verification,
//!                                         discriminator, delivery_id, payload,
//!                                         reply: Option<Reply>, cursor, interval }],
//!             config:   [ConfigField { name, service, label, help, example, format, required,
//!                                      secret, docs_url, binds }],
//!             graphs:   [Graph { name, service, description, inputs, output, nodes, edges }],
//!             verify:   Option<String>,
//!             provenance }
//! ```
//!
//! `operations` and `services` are the outbound direction; `events` and `channels` the inbound one;
//! `auth`/`default_auth` span both; `config` is what a human types before any of it runs; `graphs` is
//! a flow composed from the members above; a [`Service`]'s `roles` is the capability shape it claims;
//! and `verify` names the one cheap read that proves the whole arrangement works. See
//! `docs/designs/connector-surfaces.md` for the surface table, including which of these reach a
//! generated artifact and which currently stop here.
//!
//! Four things about it are worth reading the docs on before using it:
//!
//! - **A service is a level, not a label.** Every member belongs to exactly one [`Service`], the
//!   services partition the member set, and one that names none belongs to the reserved
//!   [`DEFAULT_SERVICE`] — which is elided from every rendered [`address`]. A service owns its base
//!   URL and its API version, because AWS versions `s3` and `bedrock-runtime` separately. It is also
//!   what claims a [`Role`] — a checked capability shape, with the provider's set *derived* as the
//!   union of its services'. See [`Connector::service_names`], [`Connector::roles`],
//!   `docs/designs/provider-services.md` and `docs/designs/provider-roles.md`.
//! - **A service has five member kinds, sharing one namespace.** An [`Operation`] is the outbound
//!   direction, an [`EventDecl`] the inbound one, and a [`ChannelBinding`] the composition of the two
//!   — it names the events it carries *and* the operation that replies to them. A [`ConfigField`] is
//!   what a human supplies before any of them run, and a [`Graph`] is a flow composed from the rest.
//!   All five project into the same address space and into flux's declaration namespace, so a name
//!   collision across kinds is a loader error — and a config field shares the namespace despite
//!   nothing *calling* it, because it shares the *host's*. [`Connector::member_names_of`] returns all
//!   five and is the definition; several doc comments elsewhere still say "three", from before the
//!   last two landed. See also [`inbound`], [`config`] and [`graph`].
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
//! [`provider::load_with_spec`] is the same call with the provider's vendored spec cache beside it,
//! and it is the whole spec front-end in one line: **spec -> patch -> validate**. It resolves the
//! file's `[spec] path` against the cache — the pin decides which document is compiled, and it is
//! checked against the declared `sha256` rather than merely labelled with it — then
//! [`openapi::ingest`] turns that document into every operation the vendor declares, the patch set
//! says which of them the connector publishes, and the result goes through exactly the validation
//! pass a hand-authored file does.
//! **Ingest selects nothing** — a pointer at a 398-operation document with no patch is a connector
//! with no operations, which is what keeps a vendor catalogue from becoming 398 LLM tools by
//! default.
//!
//! # Recording what produced an artifact
//!
//! [`Lockfile`] renders `connectors.lock`: one [`LockEntry`] per provider, holding the hashes and
//! versions `flux-connectors check` recomputes. Its contract is that unchanged inputs reproduce the
//! file byte for byte — see the [`lock`] module docs for what is hashed, what is deliberately not,
//! and why no timestamp appears anywhere in it.

mod auth;
pub mod config;
pub mod graph;
pub mod inbound;
mod ir;
pub mod lock;
pub mod openapi;
pub mod provider;

// **The address vocabulary lives in `connector-address` and is re-exported here whole** (C-407).
//
// The modules moved out of this crate; the paths did not. `connector_spec::address::Gid`,
// `connector_spec::credential::CredentialRef` and every name below still resolve, because this is a
// packaging change and an import a consumer has to edit would mean it was done wrong.
//
// The edge points down, not up: this crate *derives* addresses from a loaded connector
// ([`Connector::gid_of`], [`Connector::credential_ref_for`]) and owns none of their spelling.
// `connector-secrets` — a published host library — needed the spelling and nothing else here, and
// while the two shared a crate the derived publish closure put the whole compiler on crates.io to
// deliver it.
pub use connector_address::{
    address, credential, Error as AddressError, Gid, Oip, Pid, DEFAULT_SERVICE,
};

pub use auth::{
    AuthHazard, AuthMethod, AuthQuirks, AuthRequirement, AuthScheme, OAuth2Spec, OAuthGrant,
    OAuthRedirect, Subject, TokenEndpointQuirk,
};
pub use config::{Approval, Binding, Choice, ConfigField, Format, Level, Pin, Position};
pub use credential::{CredentialRef, InstanceId, Layout, TenantInstances, TenantLayout};
pub use graph::{
    Backoff, Compare, Condition, Edge, Graph, GraphNode, NodeKind, Port, PortRef, TextRole,
};
pub use inbound::{
    ChannelBinding, Digest, Encoding, EventDecl, FieldSource, HmacSpec, ManualSetup, Reply,
    Selector, SocketConnectSpec, Subscription, TimestampFormat, Transport, VerificationScheme,
};
pub use ir::{
    constrains_nothing, credential_handle_schema, response_location_exists, BodyEncoding,
    Connector, ErrorEnvelope, HttpMethod, Idempotency, JsonSchema, Operation, OperationDirection,
    OperationSpecSource, Pagination, Param, ParamSet, ProducedCredential, Provenance, Quirks,
    RateLimit, Risk, Role, Runtime, SemanticEffect, Service, Tag, CREDENTIAL_HANDLE_FIELD,
    FREE_FORM_BODY, MIN_REPEATABILITY_CONDITION,
};
pub use lock::{
    sha256_hex, LockEntry, LockPack, LockSpec, Lockfile, LOCKFILE_NAME, LOCKFILE_VERSION,
};
pub use openapi::{Diagnostic, Ingested, Server, ServerVariable, SpecOperation};
pub use provider::{
    IngestedDocument, LoadedProvider, Naming, NamingRule, OperationPatch, OperationSelector,
    ParamOmission, ParamPatch, ParamPosition, Patch, SpecDocument, SpecSource,
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
    /// A vendored spec document is not an OpenAPI 3.x document at all — see [`openapi::ingest`].
    ///
    /// Reserved for the whole-document failure. Everything narrower — one endpoint with an
    /// unresolvable `$ref`, one parameter with no schema — is a [`openapi::Diagnostic`] instead, so
    /// that a real vendor document's inevitable rough edges cost the offending endpoint and not the
    /// other 397.
    ///
    /// It carries no file name, because [`openapi::ingest`] takes bytes and has none to give.
    /// [`provider::load_with_spec`] is the layer that knows the path and states it.
    #[error("the vendored document is not an OpenAPI 3.x document: {reason}")]
    ParseSpec {
        /// Why it is not one this ingest can read.
        reason: String,
    },
    /// A global address (pid, gid or oip) is not well-formed — see [`address`].
    ///
    /// **Raised by [`AddressError`], not by this enum** (C-407). The parsers moved to
    /// `connector-address` with the grammar they enforce, and a second spelling of the same failure
    /// here would be two names for one thing with nothing to say which a caller gets back.
    /// [`Gid::parse`] and its siblings return [`AddressError`]; this variant folds one into the
    /// loader's own error when a component reaches this crate unparsed.
    #[error(transparent)]
    InvalidAddress(#[from] AddressError),
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
