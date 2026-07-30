//! Whether a live Vault was offered to this build — decided once, at compile time.
//!
//! This exists for one reason: `tests/vault_live.rs` needs its skip to be **visible**, and the only
//! skip libtest reports in its default output is `#[ignore]`, which is an attribute and therefore a
//! compile-time decision. Deciding at runtime instead is what C-149 removed: a test that reads the
//! environment and returns early prints `ok` / `1 passed`, and libtest captures the reason it
//! printed, so a green run could not be told apart from an exercised transport.
//!
//! So the two variables are read here, and `cfg(live_vault)` says what the answer was. `#[ignore]`
//! then carries the reason, which libtest prints on its own line.
//!
//! # This adds no network and no non-determinism to the artifacts
//!
//! `AGENTS.md` is emphatic that generation stays explicit, committed, deterministic and offline, and
//! that nothing hides network access in a `build.rs`. Nothing here opens a socket, reads a file or
//! writes one; it reads two environment variables and emits one `cfg`. What the environment selects
//! is **which tests exist**, never what the crate compiles to — no `src/` item is behind
//! `live_vault` — and `rerun-if-env-changed` makes the selection follow the environment rather than
//! going stale against it.

fn main() {
    // The build script is the whole input, so nothing else needs to invalidate it. Stating this
    // narrows the default, which would re-run on any change anywhere in the package.
    println!("cargo::rerun-if-changed=build.rs");

    // Without this, `cfg(live_vault)` is an unexpected name and `-D warnings` refuses the crate.
    println!("cargo::rustc-check-cfg=cfg(live_vault)");

    // The whole point of the exercise: setting or clearing either variable must change which tests
    // exist, so it has to invalidate this script.
    for variable in VARIABLES {
        println!("cargo::rerun-if-env-changed={variable}");
    }

    // Both, and both non-empty: an exported-but-empty variable is a shell leaving a hole behind, not
    // an offer of a server, and treating it as one would compile the leg and then panic in it.
    let offered = VARIABLES
        .iter()
        .all(|variable| std::env::var_os(variable).is_some_and(|value| !value.is_empty()));

    if offered {
        println!("cargo::rustc-cfg=live_vault");
    } else if std::env::var_os("CARGO_FEATURE_VAULT").is_some() {
        // Only when the feature is on, because only then is there a transport to leave unexercised;
        // the default gate compiles no reqwest and this would be noise. The `ignored` line in the
        // test output is the primary signal — this one reaches a reader watching the build.
        println!(
            "cargo::warning=no live Vault was offered ({} and {} are unset), so the reqwest \
             HttpTransport is UNEXERCISED by this build: tests/vault_live.rs is compiled #[ignore]d",
            VARIABLES[0], VARIABLES[1]
        );
    }
}

/// The two variables that offer a server. Spelled here and in `tests/vault_live.rs`, which asserts
/// that the skip it prints names them both.
const VARIABLES: [&str; 2] = [
    "CONNECTOR_SECRETS_VAULT_ADDR",
    "CONNECTOR_SECRETS_VAULT_TOKEN",
];
