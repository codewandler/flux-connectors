//! The one leg that needs a real server — and is *absent* rather than green when there is none.
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
//!   cargo test -p connector-secrets --features vault --test main vault_live::
//! ```
//!
//! No flag is needed: `build.rs` reads those two variables and sets `cfg(live_vault)`, so the leg
//! runs by itself when a server is offered.
//!
//! # There are exactly two paths, and neither one is a silent pass
//!
//! This leg used to decide at *runtime*: it read the variables, `eprintln!`d a reason and returned.
//! libtest captures the output of a test that returns normally, so the reason was invisible and the
//! run reported `ok` / `1 passed` — success without having talked to anything, which is precisely
//! what C-91's "skips honestly and says so, never simulated success" forbids. C-149 replaced it.
//!
//! The decision is now made at build time, so the two paths are:
//!
//! - **A Vault was offered.** The leg is compiled without `#[ignore]` and runs. If the variables
//!   then vanish before the run, it *panics* rather than skipping — it was compiled to talk to a
//!   server, so it does not get to report success for having not tried.
//! - **No Vault was offered.** The leg is compiled `#[ignore]`d with the reason attached, so
//!   libtest prints `ignored, no live Vault was offered…` on its own line and the run reports
//!   **`0 passed; 1 ignored`**. `build.rs` also emits a `cargo::warning` naming the transport as
//!   unexercised, for a reader who is watching the build rather than the test list.
//!
//! There is no third path, and
//! [`without_a_vault_the_live_leg_is_skipped_and_never_reported_as_a_pass`] holds it that way by
//! reading libtest's report of this very binary.

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
#[cfg_attr(
    not(live_vault),
    ignore = "no live Vault was offered to this build: set CONNECTOR_SECRETS_VAULT_ADDR and \
              CONNECTOR_SECRETS_VAULT_TOKEN and build again. The reqwest HttpTransport — the \
              X-Vault-Token header, the body write and the status read — is UNEXERCISED by this \
              run. The KV v2 contract itself is covered offline by the recorded transcript in \
              src/vault.rs."
)]
async fn the_http_transport_round_trips_against_a_live_dev_server() {
    let (Ok(addr), Ok(token)) = (std::env::var(ADDR), std::env::var(TOKEN)) else {
        // Only reachable when the build saw both variables and the run does not. Failing is the
        // honest answer: this leg was compiled to talk to a server, so it does not get to report
        // success for having not tried.
        panic!(
            "this leg was compiled with a live Vault offered, but {ADDR} and {TOKEN} are not both \
             set now. Set them and re-run, or build without them and the leg is skipped instead."
        );
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

/// The guard on the paragraph above: with no Vault offered, this binary must report the live leg as
/// skipped, say why, and pass nothing.
///
/// It asserts that by running **this very binary** again, filtered to the live leg alone, and
/// reading libtest's own report — because the claim being made is about what a reader of the output
/// sees, and nothing short of the real output can prove it. A filter that matched nothing reports
/// `1 filtered out` and fails these assertions rather than passing vacuously.
///
/// Compiled only when there is no Vault, which is the only situation it describes; a live build runs
/// the real leg instead.
#[cfg(not(live_vault))]
#[test]
fn without_a_vault_the_live_leg_is_skipped_and_never_reported_as_a_pass() {
    // The live leg's own name, as libtest spells it — module-qualified since C-533 merged this
    // crate's test files into one binary. A rename (or a re-merge that moves the module) that
    // missed this one leaves the filter matching nothing, which reports `1 filtered out` and fails
    // below rather than passing.
    const LIVE_LEG: &str = "vault_live::the_http_transport_round_trips_against_a_live_dev_server";

    let binary = std::env::current_exe().expect("the running test binary");
    let output = std::process::Command::new(&binary)
        .args([LIVE_LEG, "--exact"])
        .output()
        .expect("the test binary can run itself");
    let report = String::from_utf8_lossy(&output.stdout);

    assert!(
        report.contains("1 ignored"),
        "the live leg must be reported as ignored, not run:\n{report}"
    );
    assert!(
        report.contains("0 passed"),
        "a run with no Vault must report no passes — this is the `ok`/`1 passed` C-149 \
         removed:\n{report}"
    );
    assert!(
        report.contains(ADDR) && report.contains(TOKEN),
        "the skip must name the two variables that would make the leg run:\n{report}"
    );
    assert!(
        report.contains("UNEXERCISED"),
        "the skip must say what a green run therefore does not cover:\n{report}"
    );
}
