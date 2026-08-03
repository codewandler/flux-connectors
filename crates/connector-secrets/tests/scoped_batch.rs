use connector_secrets::{
    CredentialRef, CredentialScope, MemoryStore, Secret, SecretBatch, SecretStore,
};

const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET";

fn reference(instance: Option<&str>, leaf: &str) -> CredentialRef {
    match instance {
        Some(instance) => {
            CredentialRef::for_instance("tenant-a", "com.acme.api", instance, "default", leaf)
        }
        None => CredentialRef::new("tenant-a", "com.acme.api", "default", leaf),
    }
    .expect("valid reference")
}

#[tokio::test]
async fn inventory_is_scoped_and_contains_no_values() {
    let store = MemoryStore::new();
    let wanted = reference(None, "token");
    let other = CredentialRef::new("tenant-b", "com.acme.api", "default", "token")
        .expect("valid reference");
    store
        .put(&wanted, &Secret::new(SENTINEL))
        .await
        .expect("put wanted");
    store
        .put(&other, &Secret::new("OTHER-SENTINEL-NOT-A-REAL-SECRET"))
        .await
        .expect("put other");

    let scope = CredentialScope::new("tenant-a", "com.acme.api").expect("valid scope");
    assert_eq!(
        store.references(&scope).await.expect("inventory"),
        vec![wanted]
    );
}

#[tokio::test]
async fn a_checked_batch_is_all_or_nothing() {
    let store = MemoryStore::new();
    let source = reference(None, "token");
    let destination = reference(Some("0d3f79ae-b6df-4f77-8f77-438436c3b2ef"), "token");
    let absent = reference(None, "missing");
    store
        .put(&source, &Secret::new(SENTINEL))
        .await
        .expect("put source");

    let scope = CredentialScope::new("tenant-a", "com.acme.api").expect("valid scope");
    let mut batch = SecretBatch::new(scope);
    batch
        .move_secret(source.clone(), destination.clone())
        .expect("in scope");
    batch
        .move_secret(absent, reference(None, "still-missing"))
        .expect("in scope");

    store
        .apply(&batch)
        .await
        .expect_err("one bad move refuses the batch");
    assert_eq!(
        store
            .get(&source)
            .await
            .expect("source remains")
            .expose_secret(),
        SENTINEL
    );
    assert!(store.get(&destination).await.unwrap_err().is_not_found());
}

#[test]
fn a_batch_cannot_cross_its_scope() {
    let scope = CredentialScope::new("tenant-a", "com.acme.api").expect("valid scope");
    let mut batch = SecretBatch::new(scope);
    let foreign = CredentialRef::new("tenant-b", "com.acme.api", "default", "token")
        .expect("valid reference");
    assert!(batch.delete(foreign).is_err());
}
