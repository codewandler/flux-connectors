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
//!                                                                     crates/catalog/…      (C-38)
//!                                                                     site/catalog.json     (C-42)
//! ```
//!
//! Four backends, one IR. [`catalog`] renders the Rust one and [`site`] the JSON one; both read the
//! *same* per-operation Flux, emitted once, so no two of them can describe different requests.
//!
//! [`discovery`] finds the inputs, [`seam`] compiles them, [`pipeline`] compares the result against
//! the committed tree, and only then does [`artifact`] write. `diff` stops one step earlier and
//! renders instead ([`diff`]).
//!
//! # Invariants this crate holds
//!
//! - **Hermetic and offline.** `build`, `diff` and `migration-check` read committed bytes and never
//!   contact a vendor; [`net`] is the single door, and `tests/no_network.rs` proves a build never
//!   reaches it.
//! - **Deterministic.** Equal inputs produce byte-identical artifacts, so a rebuild over unchanged
//!   inputs writes nothing at all.
//! - **All-or-nothing.** Every provider is compiled before any file is written.
//! - **Explicit.** Generation is a command a human runs and reviews as a diff — never a `build.rs`.

pub mod artifact;
pub mod catalog;
pub mod cli;
pub mod core_catalog;
pub mod diff;
pub mod discovery;
mod inbound;
pub mod migration;
pub mod net;
pub mod pipeline;
pub mod png;
pub mod scaffold;
pub mod seam;
pub mod site;
pub mod status;
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
        Command::Scaffold => scaffold(invocation, out),
        Command::MigrationCheck => migration_check(invocation, out),
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

/// Check the retained native-plugin inventory against an explicitly supplied Flux checkout.
///
/// The command starts no process from that checkout. It reads `plugins/Cargo.toml` and each member
/// manifest, then derives whether a missing integration has both conformance and publication
/// evidence. This is the executable cross-repository release checklist C-505 establishes.
fn migration_check(invocation: &Invocation, out: &mut impl Write) -> Result<()> {
    let workspace = workspace_for(invocation)?;
    let flux_root = invocation.flux_root.as_deref().context(
        "`flux-connectors migration-check` needs `--flux-root <DIR>` naming a Flux checkout",
    )?;
    let report = migration::check(workspace.root(), flux_root)?;
    writeln!(
        out,
        "native plugin migration check: {} inventoried; {} legacy present; {} retired with evidence; {} support crates",
        report.inventoried,
        report.legacy_present,
        report.retired_with_evidence,
        report.support_present
    )?;
    Ok(())
}

/// Compile every provider and write what changed.
///
/// **A build refuses outright when a committed file under an artifact root is claimed by no plan**
/// (C-429), before writing anything at all. Two reasons it lands here rather than after the writes:
/// a refusal that had already written half the run would leave the tree carrying both a partial
/// build and the orphan, and the all-or-nothing property this module already holds is the one a
/// reader will assume.
fn build(invocation: &Invocation, out: &mut impl Write) -> Result<()> {
    let workspace = workspace_for(invocation)?;
    let plan = pipeline::plan_selected(
        &workspace,
        invocation.provider.as_deref(),
        invocation.service.as_deref(),
    )?;

    report_diagnostics(&plan, out)?;
    refuse_orphans(&workspace, &plan)?;

    if plan.is_up_to_date() {
        writeln!(
            out,
            "{} up to date; nothing written",
            summarize(plan.providers.len(), plan.artifacts.len())
        )?;
        return rasterize(invocation, &workspace, out);
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
    rasterize(invocation, &workspace, out)
}

/// The `--png` half of a build: outside the plan, deliberately. See [`png`] for why the raster is
/// not a checked artifact, and why an uninstalled `flux` skips instead of failing.
fn rasterize(invocation: &Invocation, workspace: &Workspace, out: &mut impl Write) -> Result<()> {
    if !invocation.png {
        return Ok(());
    }
    match png::render(workspace)? {
        png::Outcome::Written(path) => {
            writeln!(out, "rendered {}", workspace.display_path(&path).display())?
        }
        png::Outcome::Skipped(reason) => writeln!(out, "no PNG written: {reason}")?,
    }
    Ok(())
}

/// Write the provider TOML that references a vendored document — C-419.
///
/// **To `out` and nowhere else.** This is the one command whose whole safety argument is that it
/// produces text rather than a file: the author diffs and pastes, so a bad run costs nothing and the
/// reviewed artifact stays a human's. Nothing here opens a file for writing, and the emitted TOML is
/// not an artifact — it is not hashed, not in `connectors.lock`, and `diff` says nothing about it.
fn scaffold(invocation: &Invocation, out: &mut impl Write) -> Result<()> {
    let workspace = workspace_for(invocation)?;
    let Some(provider) = invocation.provider.as_deref() else {
        bail!(
            "`flux-connectors scaffold` needs a provider to scaffold: \
             `flux-connectors scaffold <PROVIDER>`\n\n{}",
            cli::USAGE
        );
    };

    let rendered = if invocation.diff {
        scaffold::render_diff(&workspace, provider)?
    } else {
        scaffold::render(&workspace, provider, &invocation.selects)?
    };
    write!(out, "{rendered}")?;
    Ok(())
}

/// Show what a build would change. Writes nothing — see [`pipeline::plan`].
fn show_diff(invocation: &Invocation, out: &mut impl Write) -> Result<()> {
    let workspace = workspace_for(invocation)?;
    let plan = pipeline::plan_selected(
        &workspace,
        invocation.provider.as_deref(),
        invocation.service.as_deref(),
    )?;
    report_diagnostics(&plan, out)?;
    write!(out, "{}", diff::render(&workspace, &plan))?;
    // After the render, not before: `diff` is a preview, and the preview is worth having even in the
    // run that fails. The non-zero exit is the point — an orphan is drift in exactly the sense
    // `connectors.lock` exists to catch, so CI must fail on it rather than print a warning nobody
    // reads (C-429).
    refuse_orphans(&workspace, &plan)
}

/// Stop, naming every committed file under an artifact root that the plan does not claim — C-429.
///
/// Silent on a scoped run, because [`pipeline::plan_selected`] leaves the list empty there: a
/// `--provider` run compiled a subset and every other provider's artifacts would read as unclaimed.
fn refuse_orphans(workspace: &Workspace, plan: &pipeline::Plan) -> Result<()> {
    if plan.orphans.is_empty() {
        return Ok(());
    }
    bail!("{}", diff::orphan_refusal(workspace, &plan.orphans))
}

/// Say what the vendored spec documents got wrong, before saying what the build did — C-4.
///
/// **Reported, never fatal.** A real vendor OpenAPI document is incomplete or wrong somewhere, and
/// ingest skips the endpoint rather than failing the connector; this is the line that keeps that
/// from being silent. It writes nothing at all when there is nothing to say, which is every
/// hand-authored connector — so the CLI's output for the current catalogue is unchanged.
fn report_diagnostics(plan: &pipeline::Plan, out: &mut impl Write) -> Result<()> {
    for diagnostic in &plan.diagnostics {
        writeln!(out, "spec: {diagnostic}")?;
    }
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
