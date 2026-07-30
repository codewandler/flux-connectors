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
use crate::core_catalog;
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
    plan_selected(workspace, only, None)
}

/// [`plan`], narrowed to one whole service of each provider it covers (C-49).
///
/// `service` is a service name or a rendered gid; an unknown one is a loud error naming what exists,
/// per provider. A service selection restricts the *contents* of every artifact to that service's
/// operations, so — exactly like a `--provider` run — it must not rewrite the repository-wide
/// documents, which are a function of a full run.
pub fn plan_selected(
    workspace: &Workspace,
    only: Option<&str>,
    service: Option<&str>,
) -> Result<Plan> {
    let providers = discovery::discover(workspace, only)?;

    let mut artifacts = Vec::new();
    let mut entries = Vec::new();
    for provider in &providers {
        let compiled = compile(workspace, provider, service)?;
        artifacts.extend(compiled.artifacts);
        entries.push(compiled.site);
    }

    // The site's catalogue covers every provider at once, so it is a function of a **full** run
    // only. A `--provider zendesk` build would have to drop the other two to write it honestly, so
    // it leaves the committed document alone instead — neither rewritten nor reported stale. This
    // is the same reasoning `crates/catalog/src/generated.rs` records for keeping its provider
    // index by hand, reached from the other direction.
    if only.is_none() && service.is_none() {
        let core = core_catalog::read_optional(workspace)?;
        artifacts.push(planned(
            workspace.site_catalog_path(),
            site::document_with_core(entries, core.clone())?,
        )?);
        if let Some(core) = &core {
            for (path, contents) in core_catalog::public_artifacts(workspace, core)? {
                artifacts.push(planned(path, contents)?);
            }
        }
        artifacts.extend(readme_images(workspace)?);
    }

    artifacts.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(Plan {
        providers: providers.into_iter().map(|p| p.name).collect(),
        artifacts,
    })
}

/// The README's syntax-highlighted images, planned like every other artifact (C-45).
///
/// This closes the gap the regex script it replaced left open in its own docstring: the image was a
/// generated artifact that **nothing checked**, so the README could disagree with the compiler for
/// as long as nobody re-ran the script by hand. Routed through [`plan`] it inherits every property
/// the pipeline already holds — `build` rewrites it, `diff` reports it stale, an unchanged image is
/// not rewritten, and `site_catalog.rs`'s whole-repo fixed-point assertion covers it for free.
///
/// A repository-level document rather than a provider artifact, so — exactly like
/// `site/catalog.json` — it is a function of a **full** run only; see the note at its call site.
/// Absent input means nothing to render, which is the ordinary case for the fixture trees the
/// integration tests build: they have `providers/` and nothing else.
fn readme_images(workspace: &Workspace) -> Result<Vec<PlannedArtifact>> {
    let path = workspace.snippet_path();
    let Some(source) = artifact::read_if_exists(&path)? else {
        return Ok(Vec::new());
    };
    connector_flux::highlight::THEMES
        .iter()
        .map(|theme| {
            planned(
                workspace.snippet_svg_path(theme.name),
                connector_flux::highlight::render_svg(&source, theme),
            )
        })
        .collect()
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
///
/// # A service-scoped run plans only that service's own files
///
/// The catalog's unit is the **provider**: `crates/catalog/src/generated/<provider>.rs` is one table
/// indexing every operation the provider publishes. Planned from a connector narrowed to one service
/// it would be *truncated* — the other service's rows silently dropped while their renderings stayed
/// on disk — which is a stale catalogue that still compiles, the worst available outcome. So a
/// `--service` run leaves every provider-unit artifact alone, exactly as a `--provider` run leaves
/// `catalog.json` alone, and for the same reason: it is not a function of what the run compiled.
fn compile(workspace: &Workspace, provider: &Provider, service: Option<&str>) -> Result<Compiled> {
    let context = || format!("provider `{}`", provider.name);

    let inputs = ProviderInputs::read(provider).with_context(context)?;
    let mut connector = seam::load(&inputs).with_context(context)?;
    if let Some(selector) = service {
        connector = seam::select_service(&connector, selector).with_context(context)?;
    }
    let emitted = seam::emit(&connector).with_context(context)?;
    let site = site::provider_entry(&connector, &emitted.operations).with_context(context)?;

    // One module and one manifest per service — the emitted unit (C-49). A `default`-only provider
    // yields exactly the two files it always did.
    let mut artifacts = Vec::new();
    for unit in emitted.services {
        artifacts.push(planned(
            workspace.service_module_path(&provider.name, &unit.service),
            unit.module,
        )?);
        artifacts.push(planned(
            workspace.service_manifest_path(&provider.name, &unit.service),
            unit.manifest,
        )?);
    }

    // The catalog's half is provider-unit — see the note above on a service-scoped run.
    if service.is_none() {
        artifacts.push(planned(
            workspace.catalog_module_path(&provider.name),
            emitted.catalog,
        )?);
        for rendering in emitted.operations {
            artifacts.push(planned(
                workspace.catalog_op_path(&provider.name, &rendering.id),
                rendering.source,
            )?);
        }
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
