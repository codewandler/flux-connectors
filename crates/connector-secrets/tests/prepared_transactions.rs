use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use connector_secrets::{
    CredentialRef, CredentialScope, FileStore, MemoryStore, PreparedSecretError,
    PreparedSecretStore, Secret, SecretBatch, SecretProposalDigest, SecretStore,
    SecretTransactionGeneration, SecretTransactionId, SecretTransactionState,
};

const SENTINEL_OLD: &str = "SENTINEL-NOT-A-REAL-SECRET-prepared-old";
const SENTINEL_NEW: &str = "SENTINEL-NOT-A-REAL-SECRET-prepared-new";

fn generation(byte: u8) -> SecretTransactionGeneration {
    SecretTransactionGeneration::from_protocol_bytes([0, 0, 0, 0, 0, 0, 0, byte])
        .expect("non-zero generation")
}

struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "connector-secrets-prepared-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn store(&self) -> std::path::PathBuf {
        self.0.join("credentials.store")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn transaction(generation: SecretTransactionGeneration, nonce: u8) -> SecretTransactionId {
    SecretTransactionId::new(generation, [nonce; 24])
}

fn digest(byte: u8) -> SecretProposalDigest {
    SecretProposalDigest::from_protocol_bytes([byte; 32])
}

fn reference() -> CredentialRef {
    CredentialRef::new("tenant-a", "com.zendesk.api", "support", "api_token").expect("valid")
}

fn replacement(secret: &str) -> SecretBatch {
    let reference = reference();
    let mut batch = SecretBatch::new(
        CredentialScope::new(reference.tenant(), reference.authority()).expect("valid scope"),
    );
    batch
        .put(reference, Secret::new(secret))
        .expect("valid mutation");
    batch
}

#[tokio::test]
async fn prepared_store_is_object_safe_and_keeps_the_candidate_invisible_until_commit() {
    let store: Arc<dyn PreparedSecretStore> = Arc::new(MemoryStore::new());
    store
        .put(&reference(), &Secret::new(SENTINEL_OLD))
        .await
        .expect("seed");
    let id = transaction(generation(1), 7);

    assert_eq!(
        store.state(id).await.expect("state"),
        SecretTransactionState::Absent
    );
    assert_eq!(
        store
            .prepare(id, digest(1), &replacement(SENTINEL_NEW))
            .await
            .expect("prepare"),
        SecretTransactionState::Prepared
    );
    assert_eq!(
        store
            .get(&reference())
            .await
            .expect("old value remains visible")
            .expose_secret(),
        SENTINEL_OLD
    );
    assert_eq!(
        store.commit(id).await.expect("commit"),
        SecretTransactionState::Committed
    );
    assert_eq!(
        store
            .get(&reference())
            .await
            .expect("new value is published")
            .expose_secret(),
        SENTINEL_NEW
    );
}

#[tokio::test]
async fn abort_before_prepare_fences_delayed_work_and_reclaim_retires_the_generation() {
    let store = MemoryStore::new();
    let first = generation(1);
    let id = transaction(first, 8);

    assert_eq!(
        store.abort(id).await.expect("abort"),
        SecretTransactionState::Absent
    );
    assert_eq!(
        store
            .prepare(id, digest(2), &replacement(SENTINEL_NEW))
            .await,
        Err(PreparedSecretError::TransactionIdReused)
    );
    store.reclaim(first).await.expect("reclaim");
    assert_eq!(store.state(id).await, Err(PreparedSecretError::Retired));
    assert_eq!(store.abort(id).await, Err(PreparedSecretError::Retired));
}

#[tokio::test]
async fn prepared_reservation_blocks_every_ordinary_mutation() {
    let store = MemoryStore::new();
    let id = transaction(generation(1), 9);
    store
        .prepare(id, digest(3), &replacement(SENTINEL_NEW))
        .await
        .expect("prepare");

    for result in [
        store.put(&reference(), &Secret::new(SENTINEL_OLD)).await,
        store.delete(&reference()).await,
        store.apply(&replacement(SENTINEL_OLD)).await,
    ] {
        assert_eq!(
            result,
            Err(connector_secrets::StoreError::Conflict {
                path: "<memory-store>".to_owned(),
                reason: "a prepared secret transaction owns the mutation slot".to_owned(),
            })
        );
    }
}

#[tokio::test]
async fn file_store_reopens_a_prepared_candidate_and_commits_the_complete_v2_image() {
    let scratch = Scratch::new("reopen");
    let path = scratch.store();
    let id = transaction(generation(1), 10);
    {
        let store = FileStore::open(&path).expect("open");
        store
            .put(&reference(), &Secret::new(SENTINEL_OLD))
            .await
            .expect("seed");
        assert_eq!(
            store
                .prepare(id, digest(4), &replacement(SENTINEL_NEW))
                .await
                .expect("prepare"),
            SecretTransactionState::Prepared
        );
        assert_eq!(
            store.get(&reference()).await.expect("old").expose_secret(),
            SENTINEL_OLD
        );
        let live = std::fs::read_to_string(&path).expect("read live");
        assert!(live.starts_with(
            "# codewandler-connector-secrets file store, v2\n# retired-through 0000000000000000\n"
        ));
        assert!(live.contains(" prepared "));
    }

    let store = FileStore::open(&path).expect("recover");
    assert_eq!(
        store.state(id).await.expect("state"),
        SecretTransactionState::Prepared
    );
    assert_eq!(
        store.commit(id).await.expect("commit"),
        SecretTransactionState::Committed
    );
    assert_eq!(
        store.get(&reference()).await.expect("new").expose_secret(),
        SENTINEL_NEW
    );
    let live = std::fs::read_to_string(&path).expect("read committed");
    assert!(live.contains(" committed "));
    assert!(!scratch.0.join(".credentials.store.prepared").exists());
}

#[test]
fn file_store_holds_a_lifetime_non_blocking_lease() {
    let scratch = Scratch::new("lease");
    let path = scratch.store();
    let first = FileStore::open(&path).expect("first opener");
    let refusal = FileStore::open(&path).expect_err("second opener must refuse");
    assert!(matches!(
        refusal,
        connector_secrets::StoreError::Conflict { .. }
    ));
    drop(first);
    FileStore::open(&path).expect("lease released on drop");
}

#[cfg(unix)]
#[test]
fn unix_lease_metadata_is_owner_only_one_link_and_never_repaired() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    let scratch = Scratch::new("lease-metadata");
    let path = scratch.store();
    drop(FileStore::open(&path).expect("initialize"));
    let lease = scratch.0.join(".credentials.store.lease");
    std::fs::set_permissions(&lease, std::fs::Permissions::from_mode(0o640)).expect("widen");
    let widened = FileStore::open(&path).expect_err("widened lease must refuse");
    assert!(matches!(
        widened,
        connector_secrets::StoreError::Denied { .. }
    ));
    assert_eq!(
        std::fs::metadata(&lease)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0o640,
        "refusal must not repair the lease"
    );
    std::fs::set_permissions(&lease, std::fs::Permissions::from_mode(0o600))
        .expect("restore fixture");
    let second_link = scratch.0.join("lease-link");
    std::fs::hard_link(&lease, &second_link).expect("plant hard link");
    let linked = FileStore::open(&path).expect_err("multi-link lease must refuse");
    assert!(matches!(
        linked,
        connector_secrets::StoreError::Denied { .. }
    ));
    assert_eq!(std::fs::metadata(&lease).expect("metadata").nlink(), 2);
}

#[tokio::test]
async fn file_store_abort_tombstone_and_retirement_survive_reopen() {
    let scratch = Scratch::new("retire");
    let path = scratch.store();
    let first = generation(1);
    let id = transaction(first, 11);
    {
        let store = FileStore::open(&path).expect("open");
        assert_eq!(
            store.abort(id).await.expect("abort"),
            SecretTransactionState::Absent
        );
    }
    {
        let store = FileStore::open(&path).expect("reopen tombstone");
        assert_eq!(
            store
                .prepare(id, digest(5), &replacement(SENTINEL_NEW))
                .await,
            Err(PreparedSecretError::TransactionIdReused)
        );
        store.reclaim(first).await.expect("reclaim");
    }
    let store = FileStore::open(&path).expect("reopen fence");
    assert_eq!(store.state(id).await, Err(PreparedSecretError::Retired));
    let live = std::fs::read_to_string(path).expect("read fence");
    assert!(live.contains("# retired-through 0000000000000001"));
    assert!(!live.contains(" aborted"));
}

#[tokio::test]
async fn exhaustive_replay_and_winner_table_is_value_free() {
    let store = MemoryStore::new();
    let first = transaction(generation(1), 20);
    let other = transaction(generation(1), 21);
    let batch = replacement(SENTINEL_NEW);

    assert_eq!(
        store.commit(first).await,
        Err(PreparedSecretError::NotPrepared)
    );
    store
        .prepare(first, digest(6), &batch)
        .await
        .expect("prepare");
    assert_eq!(
        store
            .prepare(first, digest(6), &replacement(SENTINEL_OLD))
            .await,
        Ok(SecretTransactionState::Prepared),
        "same digest replay must not inspect the replacement batch"
    );
    assert_eq!(
        store.prepare(first, digest(7), &batch).await,
        Err(PreparedSecretError::DigestMismatch)
    );
    assert_eq!(
        store.prepare(other, digest(6), &batch).await,
        Err(PreparedSecretError::Busy)
    );
    assert_eq!(store.abort(other).await, Err(PreparedSecretError::Busy));
    assert_eq!(
        store.commit(first).await,
        Ok(SecretTransactionState::Committed)
    );
    assert_eq!(
        store.commit(first).await,
        Ok(SecretTransactionState::Committed)
    );
    assert_eq!(
        store.abort(first).await,
        Err(PreparedSecretError::AlreadyCommitted)
    );
    assert_eq!(
        store
            .prepare(first, digest(6), &replacement(SENTINEL_OLD))
            .await,
        Ok(SecretTransactionState::Committed)
    );
    assert_eq!(
        store.prepare(first, digest(7), &batch).await,
        Err(PreparedSecretError::DigestMismatch)
    );

    let aborted = transaction(generation(1), 22);
    store
        .prepare(aborted, digest(8), &batch)
        .await
        .expect("prepare");
    assert_eq!(
        store.abort(aborted).await,
        Ok(SecretTransactionState::Absent)
    );
    assert_eq!(
        store.commit(aborted).await,
        Err(PreparedSecretError::TransactionIdReused)
    );
    assert_eq!(
        store.prepare(aborted, digest(8), &batch).await,
        Err(PreparedSecretError::TransactionIdReused)
    );
}

#[test]
fn concurrent_commit_and_abort_have_exactly_one_terminal_winner() {
    use std::sync::Barrier;

    let store = Arc::new(MemoryStore::new());
    let transaction = transaction(generation(1), 23);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    runtime
        .block_on(store.prepare(transaction, digest(8), &replacement(SENTINEL_NEW)))
        .expect("prepare");

    let barrier = Arc::new(Barrier::new(3));
    let commit_store = Arc::clone(&store);
    let commit_barrier = Arc::clone(&barrier);
    let commit = std::thread::spawn(move || {
        commit_barrier.wait();
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime")
            .block_on(commit_store.commit(transaction))
    });
    let abort_store = Arc::clone(&store);
    let abort_barrier = Arc::clone(&barrier);
    let abort = std::thread::spawn(move || {
        abort_barrier.wait();
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime")
            .block_on(abort_store.abort(transaction))
    });
    barrier.wait();
    let commit = commit.join().expect("commit thread");
    let abort = abort.join().expect("abort thread");
    assert!(
        matches!(
            (commit, abort),
            (
                Ok(SecretTransactionState::Committed),
                Err(PreparedSecretError::AlreadyCommitted)
            ) | (
                Err(PreparedSecretError::TransactionIdReused),
                Ok(SecretTransactionState::Absent)
            )
        ),
        "winner pair was commit={commit:?}, abort={abort:?}"
    );
}

#[tokio::test]
async fn every_acknowledged_cross_id_abort_survives_a_later_staged_commit() {
    let store = MemoryStore::new();
    let aborted = transaction(generation(1), 30);
    let committed = transaction(generation(1), 31);
    assert_eq!(
        store.abort(aborted).await,
        Ok(SecretTransactionState::Absent)
    );
    store
        .prepare(committed, digest(9), &replacement(SENTINEL_NEW))
        .await
        .expect("prepare later work");
    store.commit(committed).await.expect("commit later work");
    assert_eq!(
        store
            .prepare(aborted, digest(10), &replacement(SENTINEL_OLD))
            .await,
        Err(PreparedSecretError::TransactionIdReused)
    );
}

#[test]
fn file_store_concurrent_cross_id_calls_refuse_without_erasing_prior_tombstones() {
    let scratch = Scratch::new("file-cross-id-concurrent");
    let path = scratch.store();
    let store = Arc::new(FileStore::open(&path).expect("open"));
    let retired_candidate = transaction(generation(1), 0x31);
    let prepared = transaction(generation(1), 0x32);
    let abort_other = transaction(generation(1), 0x33);
    let prepare_other = transaction(generation(1), 0x34);
    let commit_other = transaction(generation(1), 0x35);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        store
            .put(&reference(), &Secret::new(SENTINEL_OLD))
            .await
            .expect("seed");
        assert_eq!(
            store.abort(retired_candidate).await,
            Ok(SecretTransactionState::Absent)
        );
        assert_eq!(
            store
                .prepare(prepared, digest(0x32), &replacement(SENTINEL_NEW))
                .await,
            Ok(SecretTransactionState::Prepared)
        );
    });

    let barrier = Arc::new(std::sync::Barrier::new(4));
    let abort = {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("runtime")
                .block_on(store.abort(abort_other))
        })
    };
    let prepare = {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("runtime")
                .block_on(store.prepare(prepare_other, digest(0x34), &replacement(SENTINEL_OLD)))
        })
    };
    let commit = {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("runtime")
                .block_on(store.commit(commit_other))
        })
    };
    barrier.wait();
    assert_eq!(
        abort.join().expect("abort thread"),
        Err(PreparedSecretError::Busy)
    );
    assert_eq!(
        prepare.join().expect("prepare thread"),
        Err(PreparedSecretError::Busy)
    );
    assert_eq!(
        commit.join().expect("commit thread"),
        Err(PreparedSecretError::NotPrepared)
    );

    runtime.block_on(async {
        assert_eq!(
            store.commit(prepared).await,
            Ok(SecretTransactionState::Committed)
        );
    });
    drop(store);

    let reopened = FileStore::open(&path).expect("reopen");
    runtime.block_on(async {
        assert_eq!(
            reopened
                .prepare(retired_candidate, digest(0x31), &replacement(SENTINEL_OLD),)
                .await,
            Err(PreparedSecretError::TransactionIdReused),
            "the earlier acknowledged abort must survive the staged commit"
        );
        assert_eq!(
            reopened.state(prepared).await,
            Ok(SecretTransactionState::Committed)
        );
    });
}

#[tokio::test]
async fn public_renderings_and_non_credential_files_hide_encoded_sentinels() {
    let scratch = Scratch::new("rendering-evidence");
    let path = scratch.store();
    let address = CredentialRef::new(
        "tenant-address-sentinel",
        "com.zendesk.api",
        "support",
        "path-sentinel",
    )
    .expect("valid sentinel address");
    let scope = CredentialScope::new(address.tenant(), address.authority()).expect("scope");
    let mut batch = SecretBatch::new(scope);
    batch
        .put(
            address.clone(),
            Secret::new("secret-sentinel:\"line\\break\nvalue"),
        )
        .expect("sentinel mutation");
    let id = transaction(generation(7), 0x7a);
    let proposal = digest(0x7b);
    let store = FileStore::open(&path).expect("open");
    store.prepare(id, proposal, &batch).await.expect("prepare");
    let conflict = store
        .put(&address, &Secret::new("must-not-render"))
        .await
        .expect_err("ordinary write must conflict while prepared");
    let states = [
        SecretTransactionState::Absent,
        SecretTransactionState::Prepared,
        SecretTransactionState::Committed,
    ];
    let errors = [
        PreparedSecretError::Unsupported,
        PreparedSecretError::Busy,
        PreparedSecretError::DigestMismatch,
        PreparedSecretError::TransactionIdReused,
        PreparedSecretError::NotPrepared,
        PreparedSecretError::AlreadyCommitted,
        PreparedSecretError::Retired,
        PreparedSecretError::Capacity,
        PreparedSecretError::InvalidBatch,
        PreparedSecretError::Backend,
    ];
    let error_displays = errors.map(|error| error.to_string()).join(" ");

    let public = format!(
        "{store:?} {batch:?} {id:?} {proposal:?} {conflict:?} {conflict} {states:?} {errors:?} \
         {error_displays}"
    );
    let sentinels = [
        "tenant-address-sentinel",
        "tenants/tenant-address-sentinel/com.zendesk.api/support/path-sentinel",
        "secret-sentinel:\"line\\break\nvalue",
        r#"secret-sentinel:\"line\\break\nvalue"#,
        "%73%65%63%72%65%74%2D%73%65%6E%74%69%6E%65%6C",
        "c2VjcmV0LXNlbnRpbmVs",
    ];
    for sentinel in sentinels {
        assert!(
            !public.contains(sentinel),
            "public rendering exposed {sentinel:?}"
        );
    }

    // Windows enforces the lifetime lease with a byte-range lock, so attempting to read the lease
    // through a second handle correctly fails with ERROR_LOCK_VIOLATION. Release the store before
    // inspecting the fixed, non-credential artifacts; the separate process tests pin that the lock
    // remains held for the complete FileStore lifetime.
    drop(store);

    for entry in std::fs::read_dir(&scratch.0).expect("read scratch") {
        let entry = entry.expect("directory entry");
        let entry_path = entry.path();
        let fixed_stage = scratch.0.join(".credentials.store.prepared");
        assert!(
            !entry_path
                .to_string_lossy()
                .contains("tenant-address-sentinel")
                && !entry_path.to_string_lossy().contains("secret-sentinel"),
            "a credential-derived sentinel reached a filesystem path"
        );
        if entry_path != path && entry_path != fixed_stage && entry_path.is_file() {
            let bytes = std::fs::read(&entry_path).expect("read non-credential file");
            let rendered = String::from_utf8_lossy(&bytes);
            for sentinel in sentinels {
                assert!(
                    !rendered.contains(sentinel),
                    "non-credential file exposed {sentinel:?}"
                );
            }
        }
    }
}

#[tokio::test]
async fn terminal_capacity_refuses_without_eviction_until_owner_acknowledgement() {
    let store = MemoryStore::new();
    let first = generation(1);
    for nonce in 0..connector_secrets::MAX_TERMINAL_TRANSACTIONS {
        let mut unique = [0; 24];
        unique[16..].copy_from_slice(&(nonce as u64).to_be_bytes());
        store
            .abort(SecretTransactionId::new(first, unique))
            .await
            .expect("terminal slot");
    }
    let overflow = transaction(first, 0xff);
    assert_eq!(
        store.abort(overflow).await,
        Err(PreparedSecretError::Capacity)
    );
    assert_eq!(
        store
            .prepare(overflow, digest(11), &replacement(SENTINEL_NEW))
            .await,
        Err(PreparedSecretError::Capacity)
    );
    assert_eq!(
        store.state(overflow).await,
        Ok(SecretTransactionState::Absent),
        "capacity refusal must not write a tombstone"
    );
    store.reclaim(first).await.expect("owner acknowledgement");
    assert_eq!(
        store.state(overflow).await,
        Err(PreparedSecretError::Retired)
    );
    let next = transaction(generation(2), 1);
    assert_eq!(store.abort(next).await, Ok(SecretTransactionState::Absent));
}

#[test]
fn protocol_types_are_fixed_width_nonzero_and_opaque() {
    assert!(SecretTransactionGeneration::from_protocol_bytes([0; 8]).is_none());
    let maximum = SecretTransactionGeneration::from_protocol_bytes([0xff; 8]).expect("maximum");
    assert!(
        maximum.checked_next().is_none(),
        "generation wrap must refuse"
    );
    let generation = generation(1);
    let id = SecretTransactionId::new(generation, [0xab; 24]);
    assert_eq!(&id.protocol_bytes()[..8], &generation.protocol_bytes());
    assert_eq!(&id.protocol_bytes()[8..], &[0xab; 24]);
    assert!(SecretTransactionId::from_protocol_bytes([0; 32]).is_none());
    assert_eq!(
        format!("{generation:?}"),
        "SecretTransactionGeneration(<opaque>)"
    );
    assert_eq!(format!("{id:?}"), "SecretTransactionId(<opaque>)");
    assert_eq!(
        format!("{:?}", digest(12)),
        "SecretProposalDigest(<opaque>)"
    );
    assert_eq!(
        format!("{:?}", replacement(SENTINEL_NEW)),
        "SecretBatch(<opaque>)"
    );
    assert_eq!(format!("{:?}", MemoryStore::new()), "MemoryStore(<opaque>)");
}

#[test]
fn payload_free_state_and_error_renderings_are_fixed() {
    let states = [
        (SecretTransactionState::Absent, "Absent"),
        (SecretTransactionState::Prepared, "Prepared"),
        (SecretTransactionState::Committed, "Committed"),
    ];
    for (state, expected) in states {
        assert_eq!(format!("{state:?}"), expected);
    }

    let errors = [
        (
            PreparedSecretError::Unsupported,
            "Unsupported",
            "prepared transactions are unsupported",
        ),
        (
            PreparedSecretError::Busy,
            "Busy",
            "the prepared transaction slot is busy",
        ),
        (
            PreparedSecretError::DigestMismatch,
            "DigestMismatch",
            "the proposal digest does not match",
        ),
        (
            PreparedSecretError::TransactionIdReused,
            "TransactionIdReused",
            "the transaction id was already used",
        ),
        (
            PreparedSecretError::NotPrepared,
            "NotPrepared",
            "the transaction was not prepared",
        ),
        (
            PreparedSecretError::AlreadyCommitted,
            "AlreadyCommitted",
            "the transaction was already committed",
        ),
        (
            PreparedSecretError::Retired,
            "Retired",
            "the transaction generation was retired",
        ),
        (
            PreparedSecretError::Capacity,
            "Capacity",
            "the prepared transaction store is at capacity",
        ),
        (
            PreparedSecretError::InvalidBatch,
            "InvalidBatch",
            "the secret batch is invalid",
        ),
        (
            PreparedSecretError::Backend,
            "Backend",
            "the prepared transaction backend failed",
        ),
    ];
    for (error, expected_debug, expected_display) in errors {
        assert_eq!(format!("{error:?}"), expected_debug);
        assert_eq!(error.to_string(), expected_display);
    }
}

#[tokio::test]
async fn v1_stays_byte_identical_until_the_first_transaction_use() {
    let scratch = Scratch::new("lazy-v2");
    let path = scratch.store();
    {
        let store = FileStore::open(&path).expect("open");
        store
            .put(&reference(), &Secret::new(SENTINEL_OLD))
            .await
            .expect("put v1");
    }
    let v1 = std::fs::read(&path).expect("read v1");
    {
        let store = FileStore::open(&path).expect("reopen clean v1");
        assert_eq!(
            store.get(&reference()).await.expect("read").expose_secret(),
            SENTINEL_OLD
        );
    }
    assert_eq!(std::fs::read(&path).expect("read unchanged v1"), v1);

    let store = FileStore::open(&path).expect("open for transaction");
    store
        .abort(transaction(generation(1), 40))
        .await
        .expect("first transaction use");
    assert!(std::fs::read_to_string(path)
        .expect("read v2")
        .starts_with("# codewandler-connector-secrets file store, v2\n"));
}

#[tokio::test]
async fn v2_live_and_stage_bytes_are_canonical_and_fixture_pinned() {
    let scratch = Scratch::new("v2-fixture");
    let path = scratch.store();
    let id = transaction(generation(1), 0x0a);
    let digest = digest(0x04);
    let store = FileStore::open(&path).expect("open");
    store
        .put(&reference(), &Secret::new(SENTINEL_OLD))
        .await
        .expect("seed");
    store
        .prepare(id, digest, &replacement(SENTINEL_NEW))
        .await
        .expect("prepare");

    let id_hex = format!("{}{}", "0000000000000001", "0a".repeat(24));
    let digest_hex = "04".repeat(32);
    let address = "tenants/tenant-a/com.zendesk.api/support/api_token";
    let old_hex = SENTINEL_OLD
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let new_hex = SENTINEL_NEW
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        std::fs::read_to_string(&path).expect("live"),
        format!(
            "# codewandler-connector-secrets file store, v2\n\
             # retired-through 0000000000000000\n\
             # transaction {id_hex} prepared {digest_hex}\n\
             {address} {old_hex}\n"
        )
    );
    assert_eq!(
        std::fs::read_to_string(scratch.0.join(".credentials.store.prepared")).expect("stage"),
        format!(
            "# codewandler-connector-secrets file store, v2\n\
             # retired-through 0000000000000000\n\
             # transaction {id_hex} committed {digest_hex}\n\
             {address} {new_hex}\n"
        )
    );
}

#[cfg(feature = "vault")]
#[tokio::test]
async fn vault_refuses_prepared_transactions_as_unsupported() {
    use connector_secrets::vault::{TransportError, VaultRequest, VaultResponse, VaultTransport};

    #[derive(Clone)]
    struct NeverCalled;
    #[async_trait::async_trait]
    impl VaultTransport for NeverCalled {
        async fn send(&self, _: VaultRequest<'_>) -> Result<VaultResponse, TransportError> {
            panic!("unsupported prepared transactions must not reach transport")
        }
    }

    let store = connector_secrets::VaultStore::new(
        NeverCalled,
        "https://vault.invalid",
        Secret::new("SENTINEL-NOT-A-REAL-SECRET-vault-token"),
    );
    assert_eq!(
        store.state(transaction(generation(1), 50)).await,
        Err(PreparedSecretError::Unsupported)
    );
}
