//! Compiling providers into artifacts, and deciding what that would change.
//!
//! `build` and `diff` are the same computation with different endings. [`plan`] does all of the
//! work — discover, read, load, emit, compare against what is on disk — and touches nothing;
//! [`apply`] is the only function in the crate that writes an artifact. So "diff writes nothing" is
//! a structural property, not a promise a future edit can quietly break.
//!
//! Planning everything before writing anything is also what makes a failed run safe: a provider
//! that will not compile aborts the run while the tree is still untouched.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::artifact;
use crate::discovery::{self, Provider};
use crate::seam::{self, ProviderInputs};
use crate::site::{self, ProviderEntry};
use crate::workspace::Workspace;

/// What writing a planned artifact would do to the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// No such file yet.
    Created,
    /// The file exists with different bytes.
    Modified,
    /// The file already holds exactly these bytes.
    Unchanged,
}

impl Change {
    /// Whether writing would alter the tree.
    pub fn is_change(self) -> bool {
        !matches!(self, Change::Unchanged)
    }
}

/// One artifact, compiled and compared but not yet written.
#[derive(Debug, Clone)]
pub struct PlannedArtifact {
    /// Where it belongs.
    pub path: PathBuf,
    /// The bytes a build would write.
    pub contents: String,
    /// What is there now, if anything.
    pub current: Option<String>,
    /// The verdict.
    pub change: Change,
}

/// The full result of compiling a workspace, ready to write or to render.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The providers this plan covers, in discovery order.
    pub providers: Vec<String>,
    /// Every artifact, ordered by path so output is stable across runs and platforms.
    pub artifacts: Vec<PlannedArtifact>,
}

impl Plan {
    /// The artifacts a build would create or modify.
    pub fn changes(&self) -> impl Iterator<Item = &PlannedArtifact> {
        self.artifacts
            .iter()
            .filter(|artifact| artifact.change.is_change())
    }

    /// Whether the committed artifacts already match their inputs.
    pub fn is_up_to_date(&self) -> bool {
        self.changes().next().is_none()
    }
}

/// Compile every provider (or just `only`) and compare the result against the committed tree.
///
/// Performs no writes and no network IO.
pub fn plan(workspace: &Workspace, only: Option<&str>) -> Result<Plan> {
    let providers = discovery::discover(workspace, only)?;

    let mut artifacts = Vec::new();
    let mut entries = Vec::new();
    for provider in &providers {
        let compiled = compile(workspace, provider)?;
        artifacts.extend(compiled.artifacts);
        entries.push(compiled.site);
    }

    // The site's catalogue covers every provider at once, so it is a function of a **full** run
    // only. A `--provider zendesk` build would have to drop the other two to write it honestly, so
    // it leaves the committed document alone instead — neither rewritten nor reported stale. This
    // is the same reasoning `crates/catalog/src/generated.rs` records for keeping its provider
    // index by hand, reached from the other direction.
    if only.is_none() {
        artifacts.push(planned(
            workspace.site_catalog_path(),
            site::document(entries)?,
        )?);
    }

    artifacts.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(Plan {
        providers: providers.into_iter().map(|p| p.name).collect(),
        artifacts,
    })
}

/// What compiling one provider yields: its own artifacts, and its contribution to the catalogue
/// the whole run shares.
struct Compiled {
    /// The files this provider alone owns.
    artifacts: Vec<PlannedArtifact>,
    /// This provider's entry in `site/catalog.json`, which only a full run assembles (C-42).
    site: ProviderEntry,
}

/// One provider's artifacts, compiled and compared.
///
/// Two of them ship — the module and the manifest — and the rest is the catalog's (C-38): one
/// `.flux` rendering per operation plus the generated table that embeds them. They travel through
/// the same plan on purpose. Every property the pipeline already holds then covers them for free:
/// nothing is written until everything compiles, an unchanged catalog is not rewritten, and
/// `flux-connectors diff` reports a stale rendering exactly as it reports a stale module.
///
/// The site entry rides along rather than being recomputed: it needs the same `Connector` and the
/// same renderings, and compiling twice is how a document comes to describe an operation the module
/// no longer carries.
fn compile(workspace: &Workspace, provider: &Provider) -> Result<Compiled> {
    let context = || format!("provider `{}`", provider.name);

    let inputs = ProviderInputs::read(provider).with_context(context)?;
    let connector = seam::load(&inputs).with_context(context)?;
    let emitted = seam::emit(&connector).with_context(context)?;
    let site = site::provider_entry(&connector, &emitted.operations).with_context(context)?;

    let mut artifacts = vec![
        planned(workspace.module_path(&provider.name), emitted.module)?,
        planned(workspace.manifest_path(&provider.name), emitted.manifest)?,
        planned(
            workspace.catalog_module_path(&provider.name),
            emitted.catalog,
        )?,
    ];
    for rendering in emitted.operations {
        artifacts.push(planned(
            workspace.catalog_op_path(&provider.name, &rendering.id),
            rendering.source,
        )?);
    }
    Ok(Compiled { artifacts, site })
}

fn planned(path: PathBuf, contents: String) -> Result<PlannedArtifact> {
    let current = artifact::read_if_exists(&path)?;
    let change = match &current {
        None => Change::Created,
        Some(existing) if *existing == contents => Change::Unchanged,
        Some(_) => Change::Modified,
    };
    Ok(PlannedArtifact {
        path,
        contents,
        current,
        change,
    })
}

/// Write the artifacts a plan would change, leaving unchanged ones untouched.
///
/// Returns the paths actually written. Skipping unchanged files is what keeps a rebuild from
/// churning mtimes, and it is the reason a second build is a true no-op rather than a rewrite that
/// happens to produce the same bytes.
pub fn apply(plan: &Plan) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for artifact in plan.changes() {
        artifact::write_atomic(&artifact.path, &artifact.contents)?;
        written.push(artifact.path.clone());
    }
    Ok(written)
}
