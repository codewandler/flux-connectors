//! An in-process secret store and prepared-transaction fixture.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::{
    batch, CredentialRef, CredentialScope, Layout, PreparedSecretError, PreparedSecretStore,
    Secret, SecretBatch, SecretProposalDigest, SecretStore, SecretTransactionGeneration,
    SecretTransactionId, SecretTransactionState, StoreError, TenantLayout,
    MAX_TERMINAL_TRANSACTIONS,
};

const PREPARED_CONFLICT: &str = "a prepared secret transaction owns the mutation slot";
const MEMORY_PATH: &str = "<memory-store>";

#[derive(Clone)]
enum Terminal {
    Committed(SecretProposalDigest),
    Aborted,
}

struct Prepared {
    id: SecretTransactionId,
    digest: SecretProposalDigest,
    candidate: BTreeMap<String, Secret>,
}

#[derive(Default)]
struct MemoryState {
    entries: BTreeMap<String, Secret>,
    retired_through: u64,
    terminals: BTreeMap<[u8; 32], Terminal>,
    prepared: Option<Prepared>,
}

/// A [`SecretStore`] held in memory.
pub struct MemoryStore<L = TenantLayout> {
    layout: L,
    state: Mutex<MemoryState>,
}

impl MemoryStore<TenantLayout> {
    /// An empty store using the blessed [`TenantLayout`].
    pub fn new() -> Self {
        Self::with_layout(TenantLayout)
    }
}

impl Default for MemoryStore<TenantLayout> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L: Layout> MemoryStore<L> {
    /// An empty store rendering paths through `layout`.
    pub fn with_layout(layout: L) -> Self {
        Self {
            layout,
            state: Mutex::new(MemoryState::default()),
        }
    }

    pub fn layout(&self) -> &L {
        &self.layout
    }

    pub fn path(&self, reference: &CredentialRef) -> String {
        self.layout.render(reference)
    }

    pub fn reference(&self, path: &str) -> Result<CredentialRef, StoreError> {
        self.layout
            .parse(path)
            .map_err(|reason| StoreError::Layout { reason })
    }

    pub fn paths(&self) -> Vec<String> {
        self.locked().entries.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.locked().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, MemoryState> {
        self.state.lock().unwrap_or_else(|poisoned| {
            self.state.clear_poison();
            poisoned.into_inner()
        })
    }

    fn mutation_conflict() -> StoreError {
        StoreError::Conflict {
            path: MEMORY_PATH.to_owned(),
            reason: PREPARED_CONFLICT.to_owned(),
        }
    }

    fn is_retired(state: &MemoryState, id: SecretTransactionId) -> bool {
        id.generation().value() <= state.retired_through
    }
}

impl<L> fmt::Debug for MemoryStore<L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MemoryStore(<opaque>)")
    }
}

#[async_trait]
impl<L: Layout + Send + Sync> SecretStore for MemoryStore<L> {
    async fn get(&self, reference: &CredentialRef) -> Result<Secret, StoreError> {
        let path = self.layout.render(reference);
        self.locked()
            .entries
            .get(&path)
            .cloned()
            .ok_or(StoreError::NotFound { path })
    }

    async fn put(&self, reference: &CredentialRef, secret: &Secret) -> Result<(), StoreError> {
        let mut state = self.locked();
        if state.prepared.is_some() {
            return Err(Self::mutation_conflict());
        }
        state
            .entries
            .insert(self.layout.render(reference), secret.clone());
        Ok(())
    }

    async fn delete(&self, reference: &CredentialRef) -> Result<(), StoreError> {
        let mut state = self.locked();
        if state.prepared.is_some() {
            return Err(Self::mutation_conflict());
        }
        state.entries.remove(&self.layout.render(reference));
        Ok(())
    }

    async fn references(&self, scope: &CredentialScope) -> Result<Vec<CredentialRef>, StoreError> {
        self.locked()
            .entries
            .keys()
            .map(|path| {
                self.layout
                    .parse(path)
                    .map_err(|reason| StoreError::Layout { reason })
            })
            .filter_map(|result| match result {
                Ok(reference) if scope.contains(&reference) => Some(Ok(reference)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    async fn apply(&self, mutations: &SecretBatch) -> Result<(), StoreError> {
        let mut state = self.locked();
        if state.prepared.is_some() {
            return Err(Self::mutation_conflict());
        }
        let mut candidate = state.entries.clone();
        batch::apply_to(&mut candidate, &self.layout, mutations)?;
        state.entries = candidate;
        Ok(())
    }
}

#[async_trait]
impl<L: Layout + Send + Sync> PreparedSecretStore for MemoryStore<L> {
    async fn prepare(
        &self,
        id: SecretTransactionId,
        digest: SecretProposalDigest,
        mutations: &SecretBatch,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        let mut state = self.locked();
        if Self::is_retired(&state, id) {
            return Err(PreparedSecretError::Retired);
        }
        if let Some(prepared) = &state.prepared {
            if prepared.id != id {
                return Err(PreparedSecretError::Busy);
            }
            return if prepared.digest == digest {
                Ok(SecretTransactionState::Prepared)
            } else {
                Err(PreparedSecretError::DigestMismatch)
            };
        }
        if let Some(terminal) = state.terminals.get(&id.key()) {
            return match terminal {
                Terminal::Committed(existing) if *existing == digest => {
                    Ok(SecretTransactionState::Committed)
                }
                Terminal::Committed(_) => Err(PreparedSecretError::DigestMismatch),
                Terminal::Aborted => Err(PreparedSecretError::TransactionIdReused),
            };
        }
        if state.terminals.len() >= MAX_TERMINAL_TRANSACTIONS {
            return Err(PreparedSecretError::Capacity);
        }
        let mut candidate = state.entries.clone();
        batch::apply_to(&mut candidate, &self.layout, mutations)
            .map_err(|_| PreparedSecretError::InvalidBatch)?;
        crate::file::validate_transactional_bounds(&candidate, state.terminals.len() + 1)
            .map_err(|_| PreparedSecretError::Capacity)?;
        state.prepared = Some(Prepared {
            id,
            digest,
            candidate,
        });
        Ok(SecretTransactionState::Prepared)
    }

    async fn state(
        &self,
        id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        let state = self.locked();
        if Self::is_retired(&state, id) {
            return Err(PreparedSecretError::Retired);
        }
        if state
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.id == id)
        {
            return Ok(SecretTransactionState::Prepared);
        }
        Ok(match state.terminals.get(&id.key()) {
            Some(Terminal::Committed(_)) => SecretTransactionState::Committed,
            Some(Terminal::Aborted) | None => SecretTransactionState::Absent,
        })
    }

    async fn commit(
        &self,
        id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        let mut state = self.locked();
        if Self::is_retired(&state, id) {
            return Err(PreparedSecretError::Retired);
        }
        if let Some(terminal) = state.terminals.get(&id.key()) {
            return match terminal {
                Terminal::Committed(_) => Ok(SecretTransactionState::Committed),
                Terminal::Aborted => Err(PreparedSecretError::TransactionIdReused),
            };
        }
        let Some(prepared) = state.prepared.as_ref() else {
            return Err(PreparedSecretError::NotPrepared);
        };
        if prepared.id != id {
            return Err(PreparedSecretError::NotPrepared);
        }
        let prepared = state.prepared.take().expect("checked above");
        state.entries = prepared.candidate;
        state
            .terminals
            .insert(id.key(), Terminal::Committed(prepared.digest));
        Ok(SecretTransactionState::Committed)
    }

    async fn abort(
        &self,
        id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        let mut state = self.locked();
        if Self::is_retired(&state, id) {
            return Err(PreparedSecretError::Retired);
        }
        if let Some(terminal) = state.terminals.get(&id.key()) {
            return match terminal {
                Terminal::Committed(_) => Err(PreparedSecretError::AlreadyCommitted),
                Terminal::Aborted => Ok(SecretTransactionState::Absent),
            };
        }
        if let Some(prepared) = &state.prepared {
            if prepared.id != id {
                return Err(PreparedSecretError::Busy);
            }
        }
        if state.terminals.len() >= MAX_TERMINAL_TRANSACTIONS {
            return Err(PreparedSecretError::Capacity);
        }
        crate::file::validate_transactional_bounds(&state.entries, state.terminals.len() + 1)
            .map_err(|_| PreparedSecretError::Capacity)?;
        if state
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.id == id)
        {
            state.prepared = None;
        }
        state.terminals.insert(id.key(), Terminal::Aborted);
        Ok(SecretTransactionState::Absent)
    }

    async fn reclaim(
        &self,
        through: SecretTransactionGeneration,
    ) -> Result<(), PreparedSecretError> {
        let mut state = self.locked();
        if state.prepared.is_some() {
            return Err(PreparedSecretError::Busy);
        }
        state.retired_through = state.retired_through.max(through.value());
        let fence = state.retired_through;
        state.terminals.retain(|id, _| {
            let mut generation = [0; 8];
            generation.copy_from_slice(&id[..8]);
            u64::from_be_bytes(generation) > fence
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> CredentialRef {
        CredentialRef::new("tenant-a", "com.zendesk.api", "support", "api_token").expect("valid")
    }

    #[tokio::test]
    async fn a_value_round_trips_and_then_is_gone() {
        let store = MemoryStore::new();
        let reference = reference();
        store
            .put(&reference, &Secret::new("SENTINEL-NOT-A-REAL-SECRET"))
            .await
            .expect("put");
        assert!(store.get(&reference).await.is_ok());
        store.delete(&reference).await.expect("delete");
        assert!(store.get(&reference).await.unwrap_err().is_not_found());
    }
}
