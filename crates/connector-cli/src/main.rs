//! The `flux-connectors` binary: fetch, check, build, diff, install.
//!
//! This is the only crate in the workspace permitted to touch the network — `connector-spec` and
//! `connector-flux` stay pure so they remain unit-testable offline.

use anyhow::Result;

fn main() -> Result<()> {
    // Subcommands land with the CLI stories (C-13 build/diff, C-14 fetch/check, C-15 install).
    println!("flux-connectors {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
