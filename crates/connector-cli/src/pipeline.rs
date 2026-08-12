//! Compiling providers into artifacts, and deciding what that would change.
//!
//! `build` and `diff` are the same computation with different endings. [`plan`] does all of the
//! work — discover, read, load, emit, compare against what is on disk — and touches nothing;
//! [`apply`] is the only function in the crate that writes an artifact. So "diff writes nothing" is
//! a structural property, not a promise a future edit can quietly break.
//!
//! Planning everything before writing anything is also what makes a failed run safe: a provider
//! that will not compile aborts the run while the tree is still untouched.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

/// Whether an artifact belongs to a *family* whose membership follows the inputs — C-429.
///
/// This is the half of the build that answers "what on disk did I **not** write". A build compares
/// each planned artifact against the tree; the inverse — a committed file under a directory the
/// build owns that no plan claims — had no answer at all, so a rendering whose operation was
/// deselected survived a full `build` and a `diff` reporting everything up to date. Five did, across
/// two stories, and every one was deleted by hand.
///
/// Stating it here rather than in a list of directories somewhere is the point. The roots are
/// **derived** from what the plan actually writes, and a new kind of artifact cannot reach the tree
/// without its author answering this question, because [`planned`] will not compile without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ownership {
    /// One member of a family that owns `root` outright: every file below `root` sharing an
    /// extension with some member of the family is either claimed by the plan or an orphan.
    ///
    /// The root is a *directory the build owns*, not merely the directory the file sits in. A
    /// generated file that shares its directory with hand-written ones has no family root — see
    /// [`Ownership::Singleton`].
    Family(PathBuf),
    /// A single file a full run always writes, wherever it sits.
    ///
    /// It cannot be orphaned: its membership is not a function of what is committed, so a full plan
    /// either claims it or the artifact no longer exists in the emitter at all. This is what keeps
    /// `connectors.lock` from making the repository root an artifact root — which would put
    /// `Cargo.lock` up for removal — and `crates/catalog/src/generated.rs` from doing the same to
    /// `crates/catalog/src/lib.rs`.
    Singleton,
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
    /// Whether a sibling of this file could be an orphan, and under which root — C-429.
    pub ownership: Ownership,
}

/// A committed file under an artifact root that no plan claims — C-429.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Orphan {
    /// The file on disk.
    pub path: PathBuf,
    /// The artifact root it sits under, which is what makes it judgeable.
    pub root: PathBuf,
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
    /// Committed files under an artifact root that this plan does not claim — C-429.
    ///
    /// **Empty on a scoped run, and that is not the same as "none".** `--provider` and `--service`
    /// compiled a subset, so every artifact of every provider the run never looked at would read as
    /// unclaimed, and the check would report the catalogue as garbage. It therefore runs against a
    /// whole-catalogue plan or not at all — the same hazard `connectors.lock` carries one layer
    /// down. See [`plan_selected`].
    pub orphans: Vec<Orphan>,
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

    // The schema the canonical documents validate against (C-536). Planned on every run, scoped
    // ones included: it is a constant of the generator — no provider data, so a scoped run can
    // write it honestly — and a provider-scoped build must validate its own document against the
    // schema that will hold at integration.
    artifacts.push(planned(
        workspace.document_schema_path(),
        crate::document::schema_text(),
        Ownership::Family(workspace.documents_dir()),
    )?);

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
            // One file at the repository root, always written by a full run. Its directory is the
            // repository, not an artifact root — see [`Ownership::Singleton`].
            Ownership::Singleton,
        )?);

        artifacts.push(planned(
            workspace.catalog_index_path(),
            crate::catalog::render_index(
                &providers
                    .iter()
                    .map(|provider| provider.name.clone())
                    .collect::<Vec<_>>(),
            )?,
            Ownership::Singleton,
        )?);

        let core = core_catalog::read_optional(workspace)?;
        artifacts.push(planned(
            workspace.site_catalog_path(),
            site::document_with_core(entries, core.clone())?,
            Ownership::Singleton,
        )?);
        if let Some(core) = &core {
            let root = core_catalog::public_root(workspace);
            for (path, contents) in core_catalog::public_artifacts(workspace, core)? {
                // A family: one document per record in the vendored snapshot, so a record Flux
                // retires leaves its published document behind.
                artifacts.push(planned(path, contents, Ownership::Family(root.clone()))?);
            }
        }
        artifacts.extend(readme_images(workspace)?);
    }

    artifacts.sort_by(|a, b| a.path.cmp(&b.path));
    let orphans = if whole_catalogue {
        orphaned(&artifacts)?
    } else {
        Vec::new()
    };

    Ok(Plan {
        providers: providers.into_iter().map(|p| p.name).collect(),
        artifacts,
        diagnostics,
        orphans,
    })
}

/// Every committed file under an artifact root that `artifacts` does not claim — C-429.
///
/// The roots are **derived**: each is a directory some planned artifact declared it belongs to, so
/// the set grows with the artifacts rather than with a list somebody remembers to edit. A root a
/// hand-written list forgot is an orphan class nobody would ever find, which is the same argument
/// `tests/publish_closure.rs` makes about the publish set.
///
/// The *shape* is derived the same way. A file counts only if it shares an extension with some
/// member of that root's family, which is what keeps `crates/catalog/ops/README.md` — a hand-written
/// file in a directory whose generated contents are `.flux` — out of the report. A false positive in
/// a gate is how a gate stops being read, so the check is deliberately narrower than "everything
/// under this directory".
fn orphaned(artifacts: &[PlannedArtifact]) -> Result<Vec<Orphan>> {
    let mut families: BTreeMap<&Path, BTreeSet<&OsStr>> = BTreeMap::new();
    for artifact in artifacts {
        if let Ownership::Family(root) = &artifact.ownership {
            if let Some(extension) = artifact.path.extension() {
                families.entry(root).or_default().insert(extension);
            }
        }
    }
    let claimed: BTreeSet<&Path> = artifacts
        .iter()
        .map(|artifact| artifact.path.as_path())
        .collect();

    let mut orphans = Vec::new();
    for (root, extensions) in families {
        let mut committed = Vec::new();
        collect_files(root, &mut committed)?;
        for path in committed {
            let shaped_like_an_artifact = path
                .extension()
                .is_some_and(|extension| extensions.contains(extension));
            if shaped_like_an_artifact && !claimed.contains(path.as_path()) {
                orphans.push(Orphan {
                    path,
                    root: root.to_path_buf(),
                });
            }
        }
    }
    orphans.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(orphans)
}

/// Every regular file below `dir`, recursively; an absent directory contributes nothing.
///
/// Symlinks are skipped rather than followed: a link is not a file this build wrote, and following
/// one could walk out of the root the caller reasoned about.
fn collect_files(dir: &Path, into: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        // A fixture tree that never grew this directory, which is the ordinary case for a catalogue
        // with no renderings yet.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("cannot read {}", dir.display()));
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("cannot read an entry of {}", dir.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .with_context(|| format!("cannot inspect {}", path.display()))?;
        if kind.is_dir() {
            collect_files(&path, into)?;
        } else if kind.is_file() {
            into.push(path);
        }
    }
    Ok(())
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
                // One file per compiled-in theme, and `assets/` is not an artifact root: it holds
                // the hand-maintained `readme-snippet.flux` and the brand images. `THEMES` is a
                // constant of the highlighter rather than a function of what is committed, so the
                // set cannot shrink under a build — only under an edit to that constant, which is
                // reviewed as a diff to code.
                Ownership::Singleton,
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
    // `connectors/` holds nothing but these two files per service, so it is the family's root: a
    // module whose service stopped existing has nowhere to hide there (C-429).
    let units = Ownership::Family(workspace.artifacts_dir());
    for unit in emitted.services {
        artifacts.push(planned(
            workspace.service_module_path(&provider.name, &unit.service),
            unit.module,
            units.clone(),
        )?);
        artifacts.push(planned(
            workspace.service_manifest_path(&provider.name, &unit.service),
            unit.manifest,
            units.clone(),
        )?);
    }

    // The catalog's half is provider-unit — see the note above on a service-scoped run.
    if service.is_none() {
        // The canonical document (C-536): the whole provider in one deterministic JSON file, so —
        // like the generated table — it cannot be written honestly from a service-scoped run.
        // Emission is additive: `.flux` and `.connector.toml` above are unchanged until C-540.
        artifacts.push(planned(
            workspace.document_path(&provider.name),
            crate::document::render(&connector).with_context(context)?,
            Ownership::Family(workspace.documents_dir()),
        )?);
        artifacts.push(planned(
            workspace.catalog_module_path(&provider.name),
            emitted.catalog,
            Ownership::Family(workspace.catalog_generated_dir()),
        )?);
        let renderings = Ownership::Family(workspace.catalog_ops_root());
        for rendering in emitted.operations {
            artifacts.push(planned(
                workspace.catalog_op_path(&provider.name, &rendering.id),
                rendering.source,
                renderings.clone(),
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

/// Compare one artifact against the tree.
///
/// `ownership` has no parameter default on purpose: it is the only place the build states which
/// directory it owns, and an artifact that reached the tree without stating it would be a family
/// whose orphans nothing looks for. See [`Ownership`].
fn planned(path: PathBuf, contents: String, ownership: Ownership) -> Result<PlannedArtifact> {
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
        ownership,
    })
}

/// Write the artifacts a plan would change, leaving unchanged ones untouched.
///
/// Returns the paths actually written. Skipping unchanged files is what keeps a rebuild from
/// churning mtimes, and it is the reason a second build is a true no-op rather than a rewrite that
/// happens to produce the same bytes.
///
/// **This function never removes a file**, including an orphan ([`Plan::orphans`]). `build` refuses
/// and names one instead — see `crate::build`.
pub fn apply(plan: &Plan) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for artifact in plan.changes() {
        artifact::write_atomic(&artifact.path, &artifact.contents)?;
        written.push(artifact.path.clone());
    }
    Ok(written)
}
