//! Value-free public vocabulary for crash-recoverable prepared credential transactions.

use std::fmt;
use std::num::NonZeroU64;

use async_trait::async_trait;

use crate::{SecretBatch, SecretStore};

/// Maximum terminal transaction outcomes retained before owner-acknowledged reclamation.
pub const MAX_TERMINAL_TRANSACTIONS: usize = 4096;

/// One non-zero owner-allocated reclamation generation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecretTransactionGeneration(NonZeroU64);

impl SecretTransactionGeneration {
    /// Decode the generation's eight-byte big-endian protocol representation.
    pub fn from_protocol_bytes(bytes: [u8; 8]) -> Option<Self> {
        NonZeroU64::new(u64::from_be_bytes(bytes)).map(Self)
    }

    /// Encode the generation for a provider-owned transaction protocol.
    pub fn protocol_bytes(self) -> [u8; 8] {
        self.0.get().to_be_bytes()
    }

    /// Advance without wrapping; exhaustion is a refusal.
    pub fn checked_next(self) -> Option<Self> {
        self.0
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .map(Self)
    }

    pub(crate) fn value(self) -> u64 {
        self.0.get()
    }
}

impl fmt::Debug for SecretTransactionGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretTransactionGeneration(<opaque>)")
    }
}

/// One opaque transaction identifier: generation bytes followed by a unique 192-bit nonce.
///
/// It deliberately implements neither `Display` nor serde:
///
/// ```compile_fail
/// use connector_secrets::{SecretTransactionGeneration, SecretTransactionId};
/// let generation = SecretTransactionGeneration::from_protocol_bytes([0, 0, 0, 0, 0, 0, 0, 1]).unwrap();
/// let id = SecretTransactionId::new(generation, [7; 24]);
/// let _ = format!("{id}");
/// ```
///
/// ```compile_fail
/// use connector_secrets::SecretTransactionId;
/// fn serializable<T: serde::Serialize>() {}
/// serializable::<SecretTransactionId>();
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecretTransactionId([u8; 32]);

impl SecretTransactionId {
    /// Construct an id from the provider-owned generation and coordinator-owned nonce.
    pub fn new(generation: SecretTransactionGeneration, nonce: [u8; 24]) -> Self {
        let mut bytes = [0; 32];
        bytes[..8].copy_from_slice(&generation.protocol_bytes());
        bytes[8..].copy_from_slice(&nonce);
        Self(bytes)
    }

    /// Decode protocol bytes, refusing the reserved zero generation.
    pub fn from_protocol_bytes(bytes: [u8; 32]) -> Option<Self> {
        let mut generation = [0; 8];
        generation.copy_from_slice(&bytes[..8]);
        SecretTransactionGeneration::from_protocol_bytes(generation).map(|_| Self(bytes))
    }

    /// Encode the complete opaque id for a provider-owned protocol.
    pub fn protocol_bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) fn generation(self) -> SecretTransactionGeneration {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&self.0[..8]);
        SecretTransactionGeneration::from_protocol_bytes(bytes)
            .expect("construction excludes the zero generation")
    }

    pub(crate) fn key(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for SecretTransactionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretTransactionId(<opaque>)")
    }
}

/// A caller-computed SHA-256 proposal digest whose domain this crate never interprets.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecretProposalDigest([u8; 32]);

impl SecretProposalDigest {
    /// Wrap the exact 32 protocol bytes without interpreting them.
    pub fn from_protocol_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the exact bytes for provider persistence and protocol encoding.
    pub fn protocol_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for SecretProposalDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretProposalDigest(<opaque>)")
    }
}

/// The complete public state vocabulary. Aborted work is deliberately [`Absent`](Self::Absent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretTransactionState {
    /// No live prepared or committed outcome is visible for this id.
    Absent,
    /// A complete candidate is durably staged while committed reads retain the old image.
    Prepared,
    /// The complete candidate and its terminal outcome are committed.
    Committed,
}

/// Closed, payload-free failures from [`PreparedSecretStore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PreparedSecretError {
    #[error("prepared transactions are unsupported")]
    Unsupported,
    #[error("the prepared transaction slot is busy")]
    Busy,
    #[error("the proposal digest does not match")]
    DigestMismatch,
    #[error("the transaction id was already used")]
    TransactionIdReused,
    #[error("the transaction was not prepared")]
    NotPrepared,
    #[error("the transaction was already committed")]
    AlreadyCommitted,
    #[error("the transaction generation was retired")]
    Retired,
    #[error("the prepared transaction store is at capacity")]
    Capacity,
    #[error("the secret batch is invalid")]
    InvalidBatch,
    #[error("the prepared transaction backend failed")]
    Backend,
}

/// A secret store that durably owns one prepared transaction and its bounded recovery outcomes.
///
/// The port is deliberately object-safe. Unsupported stores implement it with the default methods
/// below; callers receive a typed refusal and must never emulate preparation with point writes.
#[async_trait]
pub trait PreparedSecretStore: SecretStore {
    async fn prepare(
        &self,
        _id: SecretTransactionId,
        _digest: SecretProposalDigest,
        _batch: &SecretBatch,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        Err(PreparedSecretError::Unsupported)
    }

    async fn state(
        &self,
        _id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        Err(PreparedSecretError::Unsupported)
    }

    async fn commit(
        &self,
        _id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        Err(PreparedSecretError::Unsupported)
    }

    async fn abort(
        &self,
        _id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        Err(PreparedSecretError::Unsupported)
    }

    async fn reclaim(
        &self,
        _through: SecretTransactionGeneration,
    ) -> Result<(), PreparedSecretError> {
        Err(PreparedSecretError::Unsupported)
    }
}
