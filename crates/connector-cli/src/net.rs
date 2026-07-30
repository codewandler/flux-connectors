//! The single door to the network.
//!
//! `connector-spec` and `connector-flux` are pure by design, so the network can only ever enter the
//! workspace through this crate — and within this crate, only through here. Two commands sit on
//! opposite sides of that line:
//!
//! - **`fetch` (C-14)** is the network. It refreshes the vendored spec cache, deliberately, when a
//!   human asks.
//! - **`build`, `diff` and `check` are hermetic and offline.** They compile committed bytes into
//!   committed artifacts. A build that reached a vendor would make its output depend on the day it
//!   ran, which is precisely what the vendored cache exists to prevent.
//!
//! Keeping the door in one module is what makes the invariant testable rather than aspirational:
//! [`checkpoint`] counts and can refuse every crossing, and `tests/no_network.rs` asserts both that
//! a build never crosses and that no other module in this crate opens a socket behind its back.
//!
//! There is no transport here yet — C-14 brings the HTTP client. The accounting exists first, on
//! purpose, so that the invariant is guarded from the moment there is something to guard.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::{bail, Result};

static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static DENIED: AtomicBool = AtomicBool::new(false);

/// Announce an intent to contact `target` before doing so.
///
/// Every network operation this crate grows must call this first. Returns an error when access is
/// denied, so a caller cannot proceed by ignoring the result.
pub fn checkpoint(target: &str) -> Result<()> {
    ATTEMPTS.fetch_add(1, Ordering::SeqCst);
    if DENIED.load(Ordering::SeqCst) {
        bail!(
            "refusing to contact {target}: network access is denied in this context — `build` and \
             `diff` compile the committed spec cache and never reach a vendor"
        );
    }
    Ok(())
}

/// How many crossings have been announced in this process.
pub fn attempts() -> u64 {
    ATTEMPTS.load(Ordering::SeqCst)
}

/// Deny network access until the returned guard drops.
pub fn deny() -> Denial {
    Denial {
        previous: DENIED.swap(true, Ordering::SeqCst),
        baseline: attempts(),
    }
}

/// Denies network access for its lifetime, and reports what was attempted during it.
#[derive(Debug)]
pub struct Denial {
    previous: bool,
    baseline: u64,
}

impl Denial {
    /// Crossings announced since this guard was taken.
    pub fn attempts(&self) -> u64 {
        attempts().saturating_sub(self.baseline)
    }
}

impl Drop for Denial {
    fn drop(&mut self) {
        DENIED.store(self.previous, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_denied_checkpoint_fails_and_is_counted() {
        let denial = deny();
        let error = checkpoint("https://example.invalid/spec.json")
            .expect_err("a denied crossing must fail");
        assert!(format!("{error:#}").contains("example.invalid"));
        assert_eq!(denial.attempts(), 1);
    }
}
