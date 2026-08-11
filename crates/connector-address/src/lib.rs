//! The address vocabulary: how a provider, a service, an operation and a tenant's credential are
//! **named**.
//!
//! ```text
//! pid          com.amazonaws                                     the provider
//! gid          com.amazonaws/s3:2006-03-01                       one service of it, versioned
//! oip          com.amazonaws/s3:2006-03-01#object-get            one operation
//! credential   tenants/<tenant>/com.zendesk.api/api_token        where a tenant's secret lives
//! ```
//!
//! [`address`] owns the first three and [`credential`] the fourth; they share one grammar, one
//! reserved [`DEFAULT_SERVICE`] and one elision rule, which is why they are one crate.
//!
//! [`origin`] owns one more name of the same kind: **which server** a connection reaches, when the
//! vendor is self-managed and the connector cannot know it in advance. It is here for the reason the
//! rest of this crate is here — the compiler validates a declared origin, the runtime pack composes
//! a request and a permission subject from a supplied one, and Exchange persists and approves a
//! proposal, so a copy of the grammar in each of them is three programs that can disagree about
//! which destination was approved (C-523).
//!
//! # Why this is its own crate
//!
//! It used to be two modules of `connector-spec`, and that crate is the **compiler**: the connector
//! IR, provider TOML and OpenAPI ingest, validation and the lockfile writer. `connector-secrets` —
//! a published host library — re-exports [`CredentialRef`] and the rest, because a consumer should
//! never have to name two crates to spell one address. The publish closure is *derived* from the
//! manifests, so that one re-export dragged the whole compiler onto crates.io, permanently, to make
//! a few hundred lines of vocabulary resolve (C-407).
//!
//! An address vocabulary is a reasonable thing to have in a published API. A compiler is not. So the
//! vocabulary moved down here and the edge **inverted**: `connector-spec` is now a *consumer* of
//! this crate — [`Connector::gid_of`][gid_of] and [`credential_ref_for`][credential_ref_for] derive
//! addresses from a loaded connector — and it re-exports every name below, so
//! `connector_spec::address` and `connector_spec::credential` are unchanged for anything that
//! already spelled them that way.
//!
//! [gid_of]: https://docs.rs/codewandler-connector-spec/latest/connector_spec/struct.Connector.html
//! [credential_ref_for]: https://docs.rs/codewandler-connector-spec/latest/connector_spec/struct.Connector.html
//!
//! # What it does not do
//!
//! It holds no secret, opens no socket and reads no file. An address is a **name**; resolving one to
//! a secret is `connector-secrets`' job, and producing one from a provider definition is
//! `connector-spec`'s. This crate only says which names are well-formed and how they render — which
//! is exactly why it can be validated, round-tripped, and depended on by both sides.
//!
//! [`HttpsOrigin`] is the one name a *tenant* supplies rather than a provider file, and it does not
//! widen that boundary: it is normalized text, it stores nothing, and it says nothing about whether
//! deployment policy has approved the destination it names — that state stays with the host.
//!
//! ```
//! use connector_address::{CredentialRef, Gid, Layout, TenantLayout};
//!
//! let gid = Gid::parse("com.amazonaws/s3:2006-03-01")?;
//! assert_eq!(gid.service, "s3");
//!
//! let reference = CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token")
//!     .expect("a valid address");
//! assert_eq!(
//!     TenantLayout.render(&reference),
//!     "tenants/9f3a4b2c/com.zendesk.api/support/api_token"
//! );
//! # Ok::<(), connector_address::Error>(())
//! ```

pub mod address;
pub mod credential;
pub mod origin;

pub use address::{Gid, Oip, Pid};
pub use credential::{
    CredentialRef, CredentialScope, InstanceId, Layout, TenantInstances, TenantLayout,
};
pub use origin::{HttpsOrigin, OriginRefusal};

/// The name of the implicit service every operation belongs to unless it names another.
///
/// **Reserved.** No `[[services]]` entry may declare it (`connector-spec`'s provider loader refuses
/// one), because it is the name of the service that exists without being declared — a declaration
/// of it would be a second, contradictable definition. It is also **elided from every rendered
/// address** ([`Gid`], [`CredentialRef`]): `default` is an internal name for "this provider has one
/// API surface", and an address is a promise that would have to be broken the day the provider grows
/// a second one.
///
/// It lives here rather than in the IR because the elision is an *address* rule: both grammars in
/// this crate are written against it, and a copy on either side of the crate boundary is a copy that
/// can drift.
pub const DEFAULT_SERVICE: &str = "default";

/// An address is not well-formed.
///
/// One variant, because there is one failure: a string that does not spell an address. The text and
/// the reason are carried separately so a caller can echo the offending input back verbatim — an
/// address a user typed is worth quoting exactly as they typed it.
///
/// Only the *parsing* direction produces this. Every validator in this crate returns a plain reason
/// string instead, so `connector-spec`'s provider loader can fold the reason into its own
/// all-problems-at-once report rather than unwrapping an error to get at it.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A global address (pid, gid or oip) is not well-formed — see [`address`].
    #[error("`{address}` is not a valid address: {reason}")]
    InvalidAddress {
        /// The address as it was given.
        address: String,
        /// Why it is not one.
        reason: String,
    },
}

/// The crate's result alias.
pub type Result<T> = std::result::Result<T, Error>;
