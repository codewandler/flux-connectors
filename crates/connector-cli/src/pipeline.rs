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
use connector_spec::{LockEntry, Lockfile};

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
    /// What the vendored spec documents got wrong — C-4.
    ///
    /// In the plan rather than printed where it was found, because a plan is what both `build` and
    /// `diff` render and neither should have to re-compile to say the same thing. Empty for every
    /// hand-authored connector, which is why no committed artifact and no existing CLI output moves
    /// when this is empty.
    pub diagnostics: Vec<String>,
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

    // A scoped run compiled a subset, so it can produce no whole-catalogue artifact — the lockfile
    // included. Deciding it once, here, is what keeps the per-provider work below from paying for a
    // row nothing will read.
    let whole_catalogue = only.is_none() && service.is_none();

    let mut artifacts = Vec::new();
    let mut entries = Vec::new();
    let mut diagnostics = Vec::new();
    let mut lockfile = Lockfile::new();
    for provider in &providers {
        let compiled = compile(workspace, provider, service, whole_catalogue)?;
        if let Some(entry) = compiled.lock {
            lockfile.insert(entry);
        }
        artifacts.extend(compiled.artifacts);
        entries.push(compiled.site);
        diagnostics.extend(compiled.diagnostics);
    }

    // **The whole-catalogue artifacts.** Each covers every provider at once, so each is a function
    // of a **full** run only. A `--provider zendesk` build would have to drop the other sixteen to
    // write one honestly, so it leaves the committed documents alone instead — neither rewritten nor
    // reported stale. `docs/designs/catalog-json.md` records the rule for `catalog.json`; C-104
    // brings `crates/catalog/src/generated.rs` under it, which is what makes a provider-scoped run's
    // write set disjoint from another provider's and so lets provider stories run in parallel.
    if whole_catalogue {
        // `connectors.lock` (C-7, written by C-189). One row per provider, so it is whole-catalogue
        // for exactly the reason the index is: written from a `--provider` run it would drop every
        // other provider's row, and `check` would then report the catalogue as clean because it no
        // longer knew those providers existed. That is worse than no lockfile.
        artifacts.push(planned(
            workspace.lockfile_path(),
            lockfile
                .to_toml()
                .context("cannot render connectors.lock")?,
        )?);

        artifacts.push(planned(
            workspace.catalog_index_path(),
            crate::catalog::render_index(
                &providers
                    .iter()
                    .map(|provider| provider.name.clone())
                    .collect::<Vec<_>>(),
            )?,
        )?);

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
        diagnostics,
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
    /// What this provider's vendored document got wrong, if it has one (C-4).
    diagnostics: Vec<String>,
    /// This provider's `connectors.lock` row — `None` on a scoped run, which will not write the
    /// lockfile and so has no use for it. See [`lock_entry`].
    lock: Option<LockEntry>,
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
fn compile(
    workspace: &Workspace,
    provider: &Provider,
    service: Option<&str>,
    lock: bool,
) -> Result<Compiled> {
    let context = || format!("provider `{}`", provider.name);

    let inputs = ProviderInputs::read(provider).with_context(context)?;
    let loaded = seam::load_reported(&inputs).with_context(context)?;
    let diagnostics = loaded.diagnostics;
    let mut connector = loaded.connector;
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
    let lock = lock
        .then(|| lock_entry(workspace, &connector, &artifacts))
        .transpose()
        .with_context(context)?;

    Ok(Compiled {
        artifacts,
        site,
        diagnostics,
        lock,
    })
}

/// One provider's `connectors.lock` row: the hashes of everything that produced its artifacts.
///
/// The input hashes come from the IR — `connector-spec` computed them while loading, and verified
/// each declared `sha256` against the bytes it read. What is added here is what only this layer
/// knows: the generator's identity, and the hash of each artifact **as the plan would write it**.
///
/// Hashing the planned contents rather than the bytes on disk is what makes the lockfile a fixed
/// point *together with* the artifacts. Hashing what is committed would record a stale artifact's
/// hash as correct, so a tree with a stale module would produce a lockfile agreeing with it — and
/// `check` would call the drift clean.
///
/// The whole-catalogue artifacts are deliberately absent: a [`LockEntry`] is one provider's row, and
/// `catalog.json` belongs to no provider. They remain covered transitively — each is a function of
/// the IR whose `ir_sha256` is recorded here — and directly by
/// `crates/connector-cli/tests/catalog_artifacts.rs`.
fn lock_entry(
    workspace: &Workspace,
    connector: &seam::Connector,
    artifacts: &[PlannedArtifact],
) -> Result<LockEntry> {
    let mut entry = LockEntry::for_connector(connector, &seam::generator())
        .context("cannot record the connector in connectors.lock")?;
    for artifact in artifacts {
        entry = entry.with_artifact(
            workspace.artifact_key(&artifact.path),
            artifact.contents.as_bytes(),
        );
    }
    Ok(entry)
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
