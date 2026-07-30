//! An in-process [`SecretStore`], and the fixture every other test in this ecosystem should reach
//! for before it reaches for a server.

use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::{CredentialRef, Layout, Secret, SecretStore, StoreError, TenantLayout};

/// A [`SecretStore`] held in memory.
///
/// It exists for two reasons. The first is that a test of anything *above* the store — the Tool
/// pack's credential port, an auth assembly, a redaction check — should not need a Vault, and a
/// mock that only pretends to have a layout would let a path bug through. This one renders real
/// paths through a real [`Layout`], so it fails in the same places a real store would.
///
/// The second is that it makes the layout observable. [`paths`](Self::paths) returns exactly the
/// keys the store is holding, which is how "a non-default layout changes the path and nothing else"
/// is provable rather than assertable.
///
/// It is **not** a secure store: values sit in process memory in the clear. It is a fixture and a
/// development stand-in, and it says so here rather than in a comment somewhere downstream.
#[derive(Debug, Default)]
pub struct MemoryStore<L = TenantLayout> {
    layout: L,
    // A `Mutex` rather than an `RwLock`: the contention story of a test fixture is not interesting,
    // and one lock is one fewer thing to reason about. `BTreeMap` so `paths()` is ordered, which
    // makes an assertion over it stable.
    entries: Mutex<BTreeMap<String, Secret>>,
}

impl MemoryStore<TenantLayout> {
    /// An empty store using the blessed [`TenantLayout`].
    pub fn new() -> Self {
        Self::with_layout(TenantLayout)
    }
}

impl<L: Layout> MemoryStore<L> {
    /// An empty store rendering paths through `layout`.
    pub fn with_layout(layout: L) -> Self {
        Self {
            layout,
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    /// The layout this store renders through.
    pub fn layout(&self) -> &L {
        &self.layout
    }

    /// The path `reference` resolves to under this store's layout.
    pub fn path(&self, reference: &CredentialRef) -> String {
        self.layout.render(reference)
    }

    /// Every path currently holding a value, in order.
    ///
    /// Values are deliberately not exposed: this answers "where did it go", which is the question a
    /// layout test asks.
    pub fn paths(&self) -> Vec<String> {
        self.locked().keys().cloned().collect()
    }

    /// How many values are held.
    pub fn len(&self) -> usize {
        self.locked().len()
    }

    /// Whether the store holds nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The map, with a poisoned lock recovered rather than propagated.
    ///
    /// A poisoned mutex here means some other test panicked while holding it; the map's invariants
    /// are a `BTreeMap`'s own, so there is nothing corrupt to protect a caller from, and panicking
    /// a second time would only bury the first panic's message.
    fn locked(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Secret>> {
        self.entries.lock().unwrap_or_else(|poisoned| {
            self.entries.clear_poison();
            poisoned.into_inner()
        })
    }
}

#[async_trait]
impl<L: Layout + Send + Sync> SecretStore for MemoryStore<L> {
    async fn get(&self, reference: &CredentialRef) -> Result<Secret, StoreError> {
        let path = self.layout.render(reference);
        self.locked()
            .get(&path)
            .cloned()
            .ok_or(StoreError::NotFound { path })
    }

    async fn put(&self, reference: &CredentialRef, secret: &Secret) -> Result<(), StoreError> {
        let path = self.layout.render(reference);
        self.locked().insert(path, secret.clone());
        Ok(())
    }

    async fn delete(&self, reference: &CredentialRef) -> Result<(), StoreError> {
        let path = self.layout.render(reference);
        // Idempotent, per the trait: the absence of a value is not an error to a caller whose
        // intent is "make sure this is gone".
        self.locked().remove(&path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Obviously not a credential. Nothing in this repository commits a value shaped like a real
    /// token — a plausible placeholder has tripped GitHub push protection here before.
    const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET";

    fn reference() -> CredentialRef {
        CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token").expect("valid")
    }

    #[tokio::test]
    async fn a_value_round_trips_and_then_is_gone() {
        let store = MemoryStore::new();
        let reference = reference();

        assert!(store.get(&reference).await.unwrap_err().is_not_found());

        store
            .put(&reference, &Secret::new(SENTINEL))
            .await
            .expect("put");
        assert_eq!(
            store.get(&reference).await.expect("get").expose_secret(),
            SENTINEL
        );

        store.delete(&reference).await.expect("delete");
        assert!(store.get(&reference).await.unwrap_err().is_not_found());
    }

    /// The gap in flux's trait this crate closes: `load` returning an `Option` cannot say which of
    /// these two happened. Here the difference is in the type.
    #[test]
    fn not_stored_and_unreachable_are_different_errors() {
        let path = "tenants/9f3a4b2c/com.zendesk.api/support/api_token".to_owned();
        let missing = StoreError::NotFound { path: path.clone() };
        let down = StoreError::Unreachable {
            path,
            reason: "connection refused".to_owned(),
        };

        assert!(missing.is_not_found());
        assert!(!down.is_not_found());
        assert_ne!(missing, down);
    }

    /// Deleting what is not there is the `--clear` case, and it succeeds.
    #[tokio::test]
    async fn delete_is_idempotent() {
        let store = MemoryStore::new();
        store.delete(&reference()).await.expect("first delete");
        store.delete(&reference()).await.expect("second delete");
        assert!(store.is_empty());
    }

    /// The tenant is in the reference, not in the store: one instance serves every tenant, and two
    /// tenants' credentials for the same connector do not collide.
    #[tokio::test]
    async fn two_tenants_do_not_collide() {
        let store = MemoryStore::new();
        let first = CredentialRef::new("tenant-a", "com.zendesk.api", "support", "api_token")
            .expect("valid");
        let second = CredentialRef::new("tenant-b", "com.zendesk.api", "support", "api_token")
            .expect("valid");

        store
            .put(&first, &Secret::new("SENTINEL-TENANT-A"))
            .await
            .expect("put");
        store
            .put(&second, &Secret::new("SENTINEL-TENANT-B"))
            .await
            .expect("put");

        assert_eq!(
            store.get(&first).await.expect("get").expose_secret(),
            "SENTINEL-TENANT-A"
        );
        assert_eq!(
            store.get(&second).await.expect("get").expose_secret(),
            "SENTINEL-TENANT-B"
        );
        assert_eq!(store.len(), 2);
    }

    /// A store is usable through the trait object, which is how it is meant to be injected.
    #[tokio::test]
    async fn the_store_is_object_safe() {
        let store: std::sync::Arc<dyn SecretStore> = std::sync::Arc::new(MemoryStore::new());
        store
            .put(&reference(), &Secret::new(SENTINEL))
            .await
            .expect("put");
        assert_eq!(
            store.get(&reference()).await.expect("get").expose_secret(),
            SENTINEL
        );
    }
}
