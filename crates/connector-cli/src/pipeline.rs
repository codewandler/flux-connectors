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

    let mut artifacts = Vec::with_capacity(providers.len() * 2);
    for provider in &providers {
        artifacts.extend(compile(workspace, provider)?);
    }
    artifacts.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(Plan {
        providers: providers.into_iter().map(|p| p.name).collect(),
        artifacts,
    })
}

/// One provider's two artifacts, compiled and compared.
fn compile(workspace: &Workspace, provider: &Provider) -> Result<Vec<PlannedArtifact>> {
    let context = || format!("provider `{}`", provider.name);

    let inputs = ProviderInputs::read(provider).with_context(context)?;
    let connector = seam::load(&inputs).with_context(context)?;
    let emitted = seam::emit(&connector).with_context(context)?;

    Ok(vec![
        planned(workspace.module_path(&provider.name), emitted.module)?,
        planned(workspace.manifest_path(&provider.name), emitted.manifest)?,
    ])
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
