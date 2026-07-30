//! The one leg that needs a real server — and skips loudly rather than pretending when there is
//! none.
//!
//! Everything about Vault's KV v2 contract that this crate *decides* — the URL shape, the response
//! envelope, the status mapping, the soft-delete case — is asserted offline against a recorded
//! transcript in `src/vault.rs`. What a transcript cannot prove is that [`HttpTransport`] puts the
//! token in the right header and reads the status back, so that is what this does, against a live
//! server, when one is offered.
//!
//! Run it against a throwaway dev server:
//!
//! ```text
//! vault server -dev -dev-root-token-id=SENTINEL-NOT-A-REAL-VAULT-TOKEN
//! CONNECTOR_SECRETS_VAULT_ADDR=http://127.0.0.1:8200 \
//! CONNECTOR_SECRETS_VAULT_TOKEN=SENTINEL-NOT-A-REAL-VAULT-TOKEN \
//!   cargo test -p connector-secrets --features vault --test vault_live
//! ```
//!
//! It is **not** `#[ignore]`d, deliberately: an ignored test reports nothing, while this one prints
//! why it did not run. There is no third path where it reports success without having talked to
//! anything.

#![cfg(feature = "vault")]

use connector_secrets::vault::HttpTransport;
use connector_secrets::{CredentialRef, Secret, SecretStore, StoreError, VaultStore};

/// The address of a dev server to run against.
const ADDR: &str = "CONNECTOR_SECRETS_VAULT_ADDR";
/// The token to use. Read from the environment and never committed.
const TOKEN: &str = "CONNECTOR_SECRETS_VAULT_TOKEN";

/// Obviously not a credential, and it is the value this test writes.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET";

#[tokio::test]
async fn the_http_transport_round_trips_against_a_live_dev_server() {
    let (Ok(addr), Ok(token)) = (std::env::var(ADDR), std::env::var(TOKEN)) else {
        eprintln!(
            "skipping: no live Vault. Set {ADDR} and {TOKEN} to run this leg against a dev \
             server. The KV v2 contract itself is covered offline by the recorded transcript in \
             src/vault.rs; what is *not* covered without a server is the reqwest transport."
        );
        return;
    };

    let transport =
        HttpTransport::new(std::time::Duration::from_secs(5)).expect("an HTTP client can be built");
    let store = VaultStore::new(transport, addr, Secret::new(token));

    // A tenant id unique to this run, so a shared dev server does not have two runs colliding.
    let tenant = format!(
        "ci-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("a clock after 1970")
            .as_nanos()
    );
    let reference = CredentialRef::new(&tenant, "com.example.api", "default", "api_token")
        .expect("a valid address");

    // Clean first, so a previous failed run cannot make this one pass.
    store
        .delete(&reference)
        .await
        .expect("delete is idempotent");
    match store.get(&reference).await {
        Err(StoreError::NotFound { .. }) => {}
        other => panic!("a never-written address must be NotFound, got {other:?}"),
    }

    store
        .put(&reference, &Secret::new(SENTINEL))
        .await
        .expect("put");
    assert_eq!(
        store.get(&reference).await.expect("get").expose_secret(),
        SENTINEL
    );

    store.delete(&reference).await.expect("delete");
    match store.get(&reference).await {
        Err(StoreError::NotFound { .. }) => {}
        other => panic!("a deleted address must be NotFound, got {other:?}"),
    }
}
