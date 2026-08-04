//! A checked, scope-bound set of secret mutations.

use std::collections::{BTreeMap, BTreeSet};

use crate::{CredentialRef, CredentialScope, Layout, Secret, StoreError};

/// One atomic set of mutations within a [`CredentialScope`].
///
/// Every address is checked when the operation is added. An address may occur only once in a batch,
/// keeping the result independent of operation ordering and making a migration auditable as a set.
#[derive(Clone)]
pub struct SecretBatch {
    scope: CredentialScope,
    touched: BTreeSet<CredentialRef>,
    operations: Vec<Mutation>,
}

impl std::fmt::Debug for SecretBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretBatch(<opaque>)")
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Mutation {
    Move {
        from: CredentialRef,
        to: CredentialRef,
    },
    Put {
        at: CredentialRef,
        secret: Secret,
    },
    Delete {
        at: CredentialRef,
    },
}

impl SecretBatch {
    /// Start an empty batch constrained to `scope`.
    pub fn new(scope: CredentialScope) -> Self {
        Self {
            scope,
            touched: BTreeSet::new(),
            operations: Vec::new(),
        }
    }

    /// The boundary every operation in this batch has passed.
    pub fn scope(&self) -> &CredentialScope {
        &self.scope
    }

    /// Add a checked move. Applying it refuses a missing source or occupied destination.
    pub fn move_secret(
        &mut self,
        from: CredentialRef,
        to: CredentialRef,
    ) -> Result<&mut Self, String> {
        self.admit([&from, &to])?;
        self.touched.insert(from.clone());
        self.touched.insert(to.clone());
        self.operations.push(Mutation::Move { from, to });
        Ok(self)
    }

    /// Add a put, replacing a value already at the address.
    pub fn put(&mut self, at: CredentialRef, secret: Secret) -> Result<&mut Self, String> {
        self.admit([&at])?;
        self.touched.insert(at.clone());
        self.operations.push(Mutation::Put { at, secret });
        Ok(self)
    }

    /// Add an idempotent delete.
    pub fn delete(&mut self, at: CredentialRef) -> Result<&mut Self, String> {
        self.admit([&at])?;
        self.touched.insert(at.clone());
        self.operations.push(Mutation::Delete { at });
        Ok(self)
    }

    fn admit<'a>(
        &self,
        references: impl IntoIterator<Item = &'a CredentialRef>,
    ) -> Result<(), String> {
        for reference in references {
            if !self.scope.contains(reference) {
                return Err(format!(
                    "credential address belongs to tenant {:?}, authority {:?}, outside batch scope {:?}/{:?}",
                    reference.tenant(),
                    reference.authority(),
                    self.scope.tenant(),
                    self.scope.authority()
                ));
            }
            if self.touched.contains(reference) {
                return Err(
                    "a credential address may occur only once in an atomic batch".to_owned(),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn operations(&self) -> &[Mutation] {
        &self.operations
    }
}

pub(crate) fn apply_to<L: Layout>(
    entries: &mut BTreeMap<String, Secret>,
    layout: &L,
    batch: &SecretBatch,
) -> Result<(), StoreError> {
    for operation in batch.operations() {
        match operation {
            Mutation::Move { from, to } => {
                let source = layout.render(from);
                let destination = layout.render(to);
                if entries.contains_key(&destination) {
                    return Err(StoreError::Conflict {
                        path: destination,
                        reason: "the move destination already holds a secret".to_owned(),
                    });
                }
                let secret = entries
                    .remove(&source)
                    .ok_or(StoreError::NotFound { path: source })?;
                entries.insert(destination, secret);
            }
            Mutation::Put { at, secret } => {
                entries.insert(layout.render(at), secret.clone());
            }
            Mutation::Delete { at } => {
                entries.remove(&layout.render(at));
            }
        }
    }
    Ok(())
}
