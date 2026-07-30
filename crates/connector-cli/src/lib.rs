//! `flux-connectors`: compile committed provider definitions into committed Flux artifacts.
//!
//! The binary is a thin shell over this library so that the whole command surface is reachable from
//! an integration test — what `tests/` exercises is exactly what `main` runs.
//!
//! # The shape of a build
//!
//! ```text
//! providers/<name>.toml ──┐
//!                         ├─► seam::load ─► Connector ─► seam::emit ─► connectors/<name>.flux
//! specs/<name>/<ver>.json ┘                                           connectors/<name>.connector.toml
//! ```
//!
//! [`discovery`] finds the inputs, [`seam`] compiles them, [`pipeline`] compares the result against
//! the committed tree, and only then does [`artifact`] write. `diff` stops one step earlier and
//! renders instead ([`diff`]).
//!
//! # Invariants this crate holds
//!
//! - **Hermetic and offline.** `build` and `diff` read committed bytes and never contact a vendor;
//!   [`net`] is the single door, and `tests/no_network.rs` proves a build never reaches it.
//! - **Deterministic.** Equal inputs produce byte-identical artifacts, so a rebuild over unchanged
//!   inputs writes nothing at all.
//! - **All-or-nothing.** Every provider is compiled before any file is written.
//! - **Explicit.** Generation is a command a human runs and reviews as a diff — never a `build.rs`.

pub mod artifact;
pub mod cli;
pub mod diff;
pub mod discovery;
pub mod net;
pub mod pipeline;
pub mod seam;
pub mod workspace;

use std::io::Write;

use anyhow::{bail, Context, Result};

use crate::cli::{Command, Invocation};
use crate::workspace::Workspace;

/// Execute a parsed command line, writing user-facing output to `out`.
pub fn run(invocation: &Invocation, out: &mut impl Write) -> Result<()> {
    match invocation.command {
        Command::Build => build(invocation, out),
        Command::Diff => show_diff(invocation, out),
        Command::Check => not_yet_implemented("check", "C-14"),
        Command::Fetch => not_yet_implemented("fetch", "C-14"),
        Command::Install => not_yet_implemented("install", "C-15"),
        Command::Help => {
            writeln!(out, "{}", cli::USAGE)?;
            Ok(())
        }
        Command::Version => {
            writeln!(out, "flux-connectors {}", env!("CARGO_PKG_VERSION"))?;
            Ok(())
        }
    }
}

/// Compile every provider and write what changed.
fn build(invocation: &Invocation, out: &mut impl Write) -> Result<()> {
    let workspace = workspace_for(invocation)?;
    let plan = pipeline::plan(&workspace, invocation.provider.as_deref())?;

    if plan.is_up_to_date() {
        writeln!(
            out,
            "{} up to date; nothing written",
            summarize(plan.providers.len(), plan.artifacts.len())
        )?;
        return Ok(());
    }

    let written = pipeline::apply(&plan)?;
    for path in &written {
        writeln!(out, "wrote {}", workspace.display_path(path).display())?;
    }
    writeln!(
        out,
        "{}; {} written",
        summarize(plan.providers.len(), plan.artifacts.len()),
        written.len()
    )?;
    Ok(())
}

/// Show what a build would change. Writes nothing — see [`pipeline::plan`].
fn show_diff(invocation: &Invocation, out: &mut impl Write) -> Result<()> {
    let workspace = workspace_for(invocation)?;
    let plan = pipeline::plan(&workspace, invocation.provider.as_deref())?;
    write!(out, "{}", diff::render(&workspace, &plan))?;
    Ok(())
}

fn summarize(providers: usize, artifacts: usize) -> String {
    let provider_noun = if providers == 1 {
        "provider"
    } else {
        "providers"
    };
    let artifact_noun = if artifacts == 1 {
        "artifact"
    } else {
        "artifacts"
    };
    format!("{providers} {provider_noun}, {artifacts} {artifact_noun}")
}

fn workspace_for(invocation: &Invocation) -> Result<Workspace> {
    let root = match &invocation.root {
        Some(root) => root.clone(),
        None => std::env::current_dir().context("cannot determine the current directory")?,
    };
    Ok(Workspace::new(root))
}

/// A declared-but-unbuilt command fails loudly, naming the story that lands it.
///
/// Deliberately an error rather than a no-op: a command that exits zero without doing anything is
/// how a CI pipeline comes to believe it is checking something it is not.
fn not_yet_implemented(command: &str, story: &str) -> Result<()> {
    bail!("`flux-connectors {command}` is not yet implemented (story {story})")
}
