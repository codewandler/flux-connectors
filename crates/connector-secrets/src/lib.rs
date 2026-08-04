//! Where a connector's credential **address** resolves to a **value**.
//!
//! [`connector_address::credential`] owns the address:
//! `tenants/<tenant>/<authority>[/@instances/<uuid>][/<service>]/<credential>`, pure and validated —
//! the instance being which of a tenant's connections, present only when it holds more than one
//! (C-406), so a single-connection address is exactly what it always was. It deliberately owns
//! nothing else — `docs/vision.md`'s non-goal is load-bearing: *"A runtime. This repo compiles; flux
//! executes."* This crate is the other half, and it is a **host library**.
//!
//! # It is outside the compile path, and that is asserted
//!
//! This host library performs runtime IO: [`FileStore`] reaches the filesystem and the optional
//! Vault transport opens a socket. If `connector-cli` could reach either, then
//! *"generation is explicit, committed, deterministic and offline"* would stop being a statement
//! about the build, and `crates/connector-cli/tests/no_network.rs` — whose strongest leg is a source
//! audit of `connector-cli/src` — would no longer be able to see the violation, because the socket
//! would live in a different crate.
//!
//! So the fence is a test over the resolved dependency graph, not a convention:
//! `crates/connector-cli/tests/dependency_fence.rs`. It reads `Cargo.lock`, which records optional
//! dependencies too, so adding the edge behind a feature flag trips it as well.
//!
//! # The shape
//!
//! ```
//! use connector_secrets::{CredentialRef, MemoryStore, Secret, SecretStore, StoreError};
//!
//! # async fn example() -> Result<(), StoreError> {
//! let store = MemoryStore::new();
//! let reference = CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token")
//!     .expect("a valid address");
//!
//! store.put(&reference, &Secret::new("SENTINEL-NOT-A-REAL-SECRET")).await?;
//! let secret = store.get(&reference).await?;
//! assert_eq!(secret.expose_secret(), "SENTINEL-NOT-A-REAL-SECRET");
//!
//! store.delete(&reference).await?;
//! assert!(matches!(store.get(&reference).await, Err(StoreError::NotFound { .. })));
//! # Ok(())
//! # }
//! # tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(example()).unwrap();
//! ```
//!
//! # Three stores, and how to choose between them
//!
//! | | survives the process | what protects a value at rest | prerequisite |
//! |---|---|---|---|
//! | [`MemoryStore`] | no | the process boundary | none |
//! | [`FileStore`] | yes | Unix owner + `0700`/`0600`, or Windows `TokenUser` owner + protected owner-only DACL; **not encryption** | an owner-only local state directory |
//! | `VaultStore` | yes | Vault's own storage and policy | a Vault an operator runs |
//!
//! [`FileStore`] is the middle rung C-207 added and C-509 made portable. Its values are **not
//! encrypted**, and its module documentation distinguishes the Unix and Windows guarantees rather
//! than treating one platform's mechanism as portable. Unix root, Windows administrators and copied
//! backups remain outside the guarantee. It is the right store for one local operator and the wrong
//! one for a shared or multi-writer deployment.
//!
//! # Three gaps in flux's trait, closed deliberately
//!
//! `flux_credentials::CredentialStore` is the nearest existing thing, and [`SecretStore`] differs
//! from it in exactly three places, each a decision rather than a preference:
//!
//! - **[`delete`](SecretStore::delete) exists.** flux's trait has none, so `flux auth set --clear`
//!   bypasses the store entirely and cannot clear a Vault-backed credential.
//! - **[`get`](SecretStore::get) distinguishes "not stored" from "backend unreachable".** flux's
//!   `load` returns an `Option` and swallows transport errors with `.ok()?`, so a Vault outage is
//!   indistinguishable from an unconfigured integration — a distinction a multi-tenant deployment
//!   will want. Here they are [`StoreError::NotFound`] and [`StoreError::Unreachable`].
//! - **The tenant is in the reference**, not baked into the store instance. One store serves every
//!   tenant, and the address says whose credential is being asked for.
//!
//! C-494 adds two host guarantees beside those point operations. [`SecretStore::references`]
//! inventories addresses inside a validated [`CredentialScope`] without exposing values, and
//! [`SecretStore::apply`] commits a checked [`SecretBatch`] atomically. Both default to an explicit
//! [`StoreError::Unsupported`] refusal, because decomposing an unsupported migration into point
//! writes would expose a half-moved connector.
//!
//! # Recoverable prepared transactions
//!
//! An owning host coordinating credentials with its own value-free metadata uses the object-safe
//! [`PreparedSecretStore`] port. [`MemoryStore`] and [`FileStore`] admit one prepared batch, keep its
//! candidate invisible until commit, fence replay with owner-allocated non-zero generations and
//! bound terminal outcomes until [`PreparedSecretStore::reclaim`]. Unsupported backends refuse with
//! [`PreparedSecretError::Unsupported`]; callers must not emulate preparation with point writes.
//!
//! `FileStore` holds a non-blocking exclusive native lease for its lifetime and persists the
//! transaction ledger with credentials in v2. Released 0.19.1 writers do not participate in that
//! lease: stop all of them before the first 0.20 open. An already-open legacy process can rewrite
//! its cached v1 image and erase v2 recovery state; a fresh 0.19.1 opener safely refuses a migrated
//! v2 file.
//!
//! Adapting *to* flux's trait remains worth doing and is a separate story (C-93); it is
//! complementary, not competing.
//!
//! # Out of scope
//!
//! No expiry, no refresh, no rotation, no revocation. The store hands back a value; keeping it
//! current is the host's problem, and flux already owns substantial machinery for it. Nothing here
//! should grow a second, differently-shaped version of that.

// Native Windows security creation and descriptor inspection require direct Win32 calls. Unsafe is
// denied everywhere except the small `file::platform` module that owns those calls.
#![deny(unsafe_code)]

mod batch;
pub mod file;
mod memory;
mod secret;
mod transaction;
#[cfg(feature = "vault")]
pub mod vault;

pub use batch::SecretBatch;
pub use file::FileStore;
pub use memory::MemoryStore;
pub use secret::Secret;
pub use transaction::{
    PreparedSecretError, PreparedSecretStore, SecretProposalDigest, SecretTransactionGeneration,
    SecretTransactionId, SecretTransactionState, MAX_TERMINAL_TRANSACTIONS,
};
#[cfg(feature = "vault")]
pub use vault::VaultStore;

// The addressing, re-exported rather than redefined. There is one credential addressing scheme in
// this ecosystem and it lives in `connector-address`; a consumer of this crate should never need to
// name both crates to spell one address.
//
// **These nine names are why `connector-address` exists** (C-407). The publish closure is derived
// from the manifests, so this re-export decides what ships beside this crate: while the addressing
// lived in `connector-spec`, it was the whole compiler, permanently, so that
// `use connector_secrets::CredentialRef` resolved outside this workspace.
pub use connector_address::credential::{
    validate_instance, validate_tenant, CredentialRef, CredentialScope, InstanceId, Layout,
    TenantInstances, TenantLayout, INSTANCES_SEGMENT, MAX_TENANT, TENANTS_ROOT,
};

use async_trait::async_trait;

/// A place a tenant's credentials are kept.
///
/// The unit of addressing is a [`CredentialRef`] — an address, never a store-specific key — so an
/// implementation composes with a [`Layout`] to decide what path that address becomes. That
/// composition is the point: the client is commodity, the convention is the part worth owning, and
/// a deployment that already has a secret layout can keep it without a second addressing scheme.
///
/// # Object safety
///
/// Deliberately object-safe (`Arc<dyn SecretStore>`), because the intended binding is *injection at
/// construction*, not a global lookup.
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Read the value at `reference`.
    ///
    /// # Errors
    ///
    /// [`StoreError::NotFound`] when the backend answered and nothing is stored there;
    /// [`StoreError::Unreachable`] when it did not answer at all. Telling those apart is the whole
    /// reason this returns a `Result` rather than an `Option`, so an implementation must never
    /// collapse a transport failure into "not configured".
    async fn get(&self, reference: &CredentialRef) -> Result<Secret, StoreError>;

    /// Write `secret` at `reference`, replacing whatever was there.
    ///
    /// # Errors
    ///
    /// Any of [`StoreError`] except [`NotFound`](StoreError::NotFound), which a write cannot
    /// produce.
    async fn put(&self, reference: &CredentialRef, secret: &Secret) -> Result<(), StoreError>;

    /// Remove whatever is stored at `reference`.
    ///
    /// **Idempotent**: deleting an address that holds nothing is `Ok(())`, not
    /// [`NotFound`](StoreError::NotFound). This is not a softening of the rule above — it is what
    /// the backend can actually support. Vault's `DELETE` on a KV v2 metadata path answers `204`
    /// whether or not the secret existed, so a store that reported `NotFound` here would have to
    /// read first, which is both an extra round trip and a race. The caller that matters is
    /// "clear this tenant's credential", and it wants idempotence.
    ///
    /// # Errors
    ///
    /// Any of [`StoreError`] except [`NotFound`](StoreError::NotFound), per the above.
    async fn delete(&self, reference: &CredentialRef) -> Result<(), StoreError>;

    /// List the addresses inside `scope`, never their values.
    ///
    /// Stores without a proven listing policy refuse explicitly; a caller must not infer an empty
    /// inventory from that refusal.
    async fn references(&self, _scope: &CredentialScope) -> Result<Vec<CredentialRef>, StoreError> {
        Err(StoreError::Unsupported {
            operation: "references".to_owned(),
            reason: "this secret store cannot enumerate a credential scope".to_owned(),
        })
    }

    /// Apply every mutation in `batch` atomically, or expose none of them.
    async fn apply(&self, _batch: &SecretBatch) -> Result<(), StoreError> {
        Err(StoreError::Unsupported {
            operation: "atomic batch".to_owned(),
            reason: "this secret store cannot guarantee atomic mutation".to_owned(),
        })
    }
}

/// What a [`SecretStore`] can fail with.
///
/// Every variant that names a location names the rendered `path`, not the [`CredentialRef`], because
/// the path is what an operator needs in order to go and look — and because a reference renders
/// differently under each [`Layout`], so quoting the address alone would send them to the wrong
/// place. [`Layout`](Self::Layout) is the exception, and only because there is no rendered path in
/// that case: the path was the *input*, and the layout's own message quotes it. None of them carries
/// a value: a `StoreError` is safe to log.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The backend answered, and there is nothing stored at that path.
    ///
    /// Distinct from [`Unreachable`](Self::Unreachable) on purpose: this one means "this tenant has
    /// not connected that integration", and the other means "we cannot say".
    #[error("no secret is stored at {path}")]
    NotFound {
        /// The path the store looked at.
        path: String,
    },

    /// The backend could not be reached, or did not answer in a usable way.
    ///
    /// A retry may succeed. Never present this to a caller as "not configured".
    #[error("the secret store at {path} is unreachable: {reason}")]
    Unreachable {
        /// The path the store was trying to reach.
        path: String,
        /// What went wrong on the wire.
        reason: String,
    },

    /// The backend answered and refused: the store's own credentials are missing, expired or not
    /// authorised for that path.
    #[error("the secret store refused access to {path}: {reason}")]
    Denied {
        /// The path access was refused to.
        path: String,
        /// What the backend said.
        reason: String,
    },

    /// The backend answered with something this client cannot interpret — a response shape it does
    /// not recognise, or a status it has no meaning for.
    ///
    /// Separate from [`Unreachable`](Self::Unreachable) because retrying will not help.
    #[error("the secret store returned an unusable response for {path}: {reason}")]
    Backend {
        /// The path in question.
        path: String,
        /// Why the response could not be used.
        reason: String,
    },

    /// The configured [`Layout`] could not recover the address from a path.
    ///
    /// A custom layout is allowed to refuse a path it cannot spell; that is stated in [`Layout`]'s
    /// contract as the alternative to guessing. [`Layout::render`] is infallible, so
    /// [`Layout::parse`] is the only place a layout can refuse, and [`MemoryStore::reference`] and
    /// `VaultStore::reference` are the two calls that raise this. That pairing is deliberate: a
    /// variant no store can construct is a promise that does not hold, and this one used to be
    /// exactly that (C-149).
    #[error("the layout could not resolve the address: {reason}")]
    Layout {
        /// The layout's own explanation.
        reason: String,
    },

    /// A checked mutation conflicts with the state already stored at its address.
    #[error("the secret store cannot apply a mutation at {path}: {reason}")]
    Conflict {
        /// The rendered address in conflict.
        path: String,
        /// The checked precondition that failed.
        reason: String,
    },

    /// The backend cannot provide a requested guarantee.
    #[error("the secret store does not support {operation}: {reason}")]
    Unsupported {
        /// The store operation that was requested.
        operation: String,
        /// Why this backend cannot honour it.
        reason: String,
    },
}

impl StoreError {
    /// Whether this is [`NotFound`](Self::NotFound) — the "this tenant has not connected it" case.
    ///
    /// A named predicate because the distinction is the reason the error type exists, and because a
    /// caller reaching for it should not have to spell out a match over a growing enum.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }
}
