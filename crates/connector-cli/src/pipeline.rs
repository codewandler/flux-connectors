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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

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
    plan_at_width(workspace, only, service, None)
}

/// [`plan_selected`], compiling `workers` providers at a time; `None` picks one per available core.
///
/// The width is a parameter because it is what the determinism contract is stated *over*: a width
/// of 1 spawns no thread and is exactly the sequential build this pipeline was until C-544, so
/// `tests::the_plan_is_identical_at_every_compile_width` can compare every concurrent plan against
/// it on the same inputs. Nothing outside this module chooses a width — [`plan_selected`] is the
/// whole public surface, and it passes `None`.
fn plan_at_width(
    workspace: &Workspace,
    only: Option<&str>,
    service: Option<&str>,
    workers: Option<usize>,
) -> Result<Plan> {
    let providers = discovery::discover(workspace, only)?;

    // A scoped run compiled a subset, so it can produce no whole-catalogue artifact — the lockfile
    // included. Deciding it once, here, is what keeps the per-provider work below from paying for a
    // row nothing will read.
    let whole_catalogue = only.is_none() && service.is_none();

    let mut artifacts = Vec::new();

    // The schema the canonical documents validate against (C-536). Planned on every run, scoped
    // ones included: it is a constant of the generator — no provider data, so a scoped run can
    // write it honestly — and a provider-scoped build must validate its own document against the
    // schema that will hold at integration.
    artifacts.push(planned(
        workspace.document_schema_path(),
        crate::document::schema_text(),
        Ownership::Family(workspace.documents_dir()),
    )?);

    let workers = workers.unwrap_or_else(|| worker_count(providers.len()));
    let Merged {
        artifacts: per_provider,
        entries,
        diagnostics,
        mut lockfile,
    } = merge(compile_all(
        workspace,
        &providers,
        service,
        whole_catalogue,
        workers,
    )?);
    artifacts.extend(per_provider);

    // **The whole-catalogue artifacts.** Each covers every provider at once, so each is a function
    // of a **full** run only. A `--provider zendesk` build would have to drop the other sixteen to
    // write one honestly, so it leaves the committed documents alone instead — neither rewritten nor
    // reported stale. `docs/designs/catalog-json.md` records the rule for `catalog.json`; C-104
    // brings `crates/catalog/src/generated.rs` under it, which is what makes a provider-scoped run's
    // write set disjoint from another provider's and so lets provider stories run in parallel.
    if whole_catalogue {
        // The catalog pack (C-537): every canonical document, compiled into the one distributable
        // file `crates/catalog-reader` embeds. Whole-catalogue for the same reason the index is —
        // a pack written from a scoped run would silently drop every provider the run never
        // compiled — and compiled from the *planned* document contents rather than the committed
        // files, so pack and documents are a fixed point together, exactly as the lockfile is
        // with the artifacts it hashes.
        //
        // Before the lockfile below on purpose: the lockfile records the pack's hash, so the pack
        // has to exist first.
        let pack_text = {
            let documents_dir = workspace.documents_dir();
            let mut documents: Vec<(&str, &str)> = artifacts
                .iter()
                .filter_map(|artifact| {
                    let provider = artifact
                        .path
                        .parent()
                        .filter(|parent| *parent == documents_dir)
                        .and_then(|_| artifact.path.file_name())
                        .and_then(std::ffi::OsStr::to_str)
                        .and_then(|name| {
                            name.strip_suffix(&format!(".{}", crate::workspace::DOCUMENT_SUFFIX))
                        })?;
                    Some((provider, artifact.contents.as_str()))
                })
                .collect();
            documents.sort_by(|a, b| a.0.cmp(b.0));
            crate::pack::compile(&documents).context("cannot compile the catalog pack")?
        };
        lockfile.set_pack(connector_spec::LockPack {
            path: workspace.artifact_key(&workspace.pack_path()),
            schema_version: crate::document::SCHEMA_VERSION,
            sha256: connector_spec::sha256_hex(pack_text.as_bytes()),
        });
        artifacts.push(planned(
            workspace.pack_path(),
            pack_text,
            // One file a full run always writes, inside the reader crate; `crates/catalog-reader`
            // is not an artifact root — it holds the hand-written reader — so the pack cannot be
            // orphaned, only replaced. See [`Ownership::Singleton`].
            Ownership::Singleton,
        )?);

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

/// Every provider's contribution to the run, folded back in **provider order** (C-544).
///
/// A named step rather than the tail of the loop that produced it, because the order is the
/// contract. [`compile_all`] compiles the providers concurrently, so the order they *finish* in is
/// a property of the machine and of nothing else; folding in that order would hand it straight to
/// [`entries`](Self::entries) and [`diagnostics`](Self::diagnostics), neither of which is sorted
/// afterwards by anything. Taking a `Vec<Compiled>` that is already in provider order and folding
/// it in one direction is what keeps every output a function of the inputs alone.
struct Merged {
    /// Every provider's own artifacts, concatenated in provider order.
    ///
    /// The plan sorts these by path before anyone reads them, so this order reaches no artifact —
    /// but it decides which of two equal paths a stable sort keeps, and it is the order the pack
    /// is compiled from before that sort happens.
    artifacts: Vec<PlannedArtifact>,
    /// One `catalog.json` entry per provider, in provider order.
    ///
    /// **This is the order the published document carries.** [`site::document_with_core`]
    /// serialises the entries as it is handed them and sorts nothing, so a fold in completion order
    /// would publish a provider array whose order was a property of the scheduler.
    entries: Vec<ProviderEntry>,
    /// What the vendored documents got wrong, in provider order — the order `build` and `diff`
    /// print them in.
    diagnostics: Vec<String>,
    /// The `connectors.lock` rows. [`Lockfile::insert`] holds its own sorted order, so the row
    /// order is already independent of this fold; it is folded here in provider order anyway,
    /// because a determinism contract that relies on a callee's incidental sort is one refactor
    /// from being false.
    lockfile: Lockfile,
}

/// Fold the per-provider results into the run, **in the order given**.
///
/// Deliberately total and order-preserving: it decides nothing and sorts nothing, so the only thing
/// that can make the output non-deterministic is being handed the results in a non-deterministic
/// order — which is precisely what [`compile_all`] guarantees it is not, and what
/// `tests::folding_in_completion_order_would_move_a_published_artifact` seeds the opposite of.
fn merge(compiled: Vec<Compiled>) -> Merged {
    let mut merged = Merged {
        artifacts: Vec::new(),
        entries: Vec::new(),
        diagnostics: Vec::new(),
        lockfile: Lockfile::new(),
    };
    for provider in compiled {
        if let Some(entry) = provider.lock {
            merged.lockfile.insert(entry);
        }
        merged.artifacts.extend(provider.artifacts);
        merged.entries.push(provider.site);
        merged.diagnostics.extend(provider.diagnostics);
    }
    merged
}

/// How many providers to compile at once: one per core this process may use, never more than there
/// are providers and never zero.
///
/// [`std::thread::available_parallelism`] reports the affinity- and cgroup-aware figure rather than
/// the machine's core count, so a one-core container gets 1 — which is [`compile_all`]'s sequential
/// branch, spawning no thread at all. An error means the platform will not say, and the honest
/// answer to that is the sequential build rather than a guess.
fn worker_count(providers: usize) -> usize {
    thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .clamp(1, providers.max(1))
}

/// Compile every provider, up to `workers` at a time, returning the results **in `providers`
/// order** whatever order they finished in (C-544).
///
/// The providers are independent: each reads its own definition and its own vendored documents, and
/// — because planning writes nothing — none of them can observe another. So the sequential loop
/// this replaced was 55 compiles on one core, and it was the floor under every `build`, `diff` and
/// whole-tree fixed-point test in the suite.
///
/// **Two properties carry the byte-determinism this repository's contract requires, and both are
/// structural rather than a promise a later edit can quietly break:**
///
/// 1. *Order is restored by index.* Each worker records the index it claimed beside its result, and
///    the sort below is the only thing that decides the order the caller sees. Nothing downstream
///    can observe which provider finished first — see [`Merged`] for what would inherit it.
/// 2. *A failure is the first one in provider order.* Collecting into a `Result<Vec<_>>` yields the
///    earliest `Err` in that restored order, which is the provider a sequential run would have
///    stopped at, carrying the message it carries today. A concurrent run does compile providers a
///    sequential one would never have reached; compiling is pure and writes nothing, so the only
///    cost of discarding their results is the work.
fn compile_all(
    workspace: &Workspace,
    providers: &[Provider],
    service: Option<&str>,
    lock: bool,
    workers: usize,
) -> Result<Vec<Compiled>> {
    if workers <= 1 {
        return providers
            .iter()
            .map(|provider| compile(workspace, provider, service, lock))
            .collect();
    }

    // The shared cursor is the whole of the scheduling: a worker claims the next index and keeps
    // claiming until they run out, so one slow provider — babelforce compiles five vendored
    // documents — cannot leave the rest queued behind it.
    let next = AtomicUsize::new(0);
    let batches = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let next = &next;
                scope.spawn(move || {
                    let mut claimed = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(provider) = providers.get(index) else {
                            return claimed;
                        };
                        claimed.push((index, compile(workspace, provider, service, lock)));
                    }
                })
            })
            // Every worker is spawned before any is joined; collecting the handles is what makes
            // that true, so this is not a needless collect.
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                // A panic inside a provider's compile is re-raised here rather than reported as a
                // join error, so it surfaces exactly as it did when the compile ran on this thread.
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect::<Vec<_>>()
    });

    let mut claimed: Vec<(usize, Result<Compiled>)> = batches.into_iter().flatten().collect();
    // The one line that decides the caller's order, and it reads the index the provider had — never
    // anything about when its result arrived.
    claimed.sort_by_key(|(index, _)| *index);
    claimed.into_iter().map(|(_, result)| result).collect()
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    use crate::artifact::tests::{scratch, Scratch};

    /// A hand-authored provider definition the real loader accepts and the real emitter compiles,
    /// carrying `operations` operations.
    ///
    /// The count is a parameter so a fixture can be **uneven**. One provider that takes visibly
    /// longer than its siblings is what makes the order the providers finish in differ from the
    /// order they were discovered in, and that difference is the whole of the hazard these tests
    /// are about: with equal costs a completion-ordered plan could coincide with a provider-ordered
    /// one by luck and the assertions below would prove nothing.
    fn definition(id: &str, operations: usize) -> String {
        let mut toml = format!(
            "id = \"{id}\"\n\
             vendor = \"{id} Inc.\"\n\
             base_url = \"https://api.{id}.example\"\n\
             description = \"A hand-authored fixture connector.\"\n"
        );
        for member in ('a'..).take(operations) {
            toml.push_str(&format!(
                "\n\
                 [[operations]]\n\
                 id = \"{id}-{member}-get\"\n\
                 method = \"GET\"\n\
                 direction = \"read\"\n\
                 path = \"/v1/{member}/{{thing_id}}\"\n\
                 description = \"Fetch one thing.\"\n\
                 risk = \"low\"\n\
                 idempotency = \"idempotent\"\n\
                 \n\
                 [[operations.params.path]]\n\
                 name = \"thing_id\"\n\
                 description = \"The thing to fetch.\"\n\
                 required = true\n\
                 schema = {{ type = \"integer\" }}\n"
            ));
        }
        toml
    }

    /// A workspace holding `providers/` and nothing else, removed when it drops.
    struct Fixture {
        scratch: Scratch,
    }

    impl Fixture {
        /// A fixture with one provider per `(name, operations)` pair.
        fn with(label: &str, providers: &[(&str, usize)]) -> Self {
            let scratch = scratch(label);
            fs::create_dir_all(scratch.join(crate::workspace::PROVIDERS_DIR))
                .expect("create the fixture's providers directory");
            let fixture = Fixture { scratch };
            for (name, operations) in providers {
                fixture.write_provider(name, &definition(name, *operations));
            }
            fixture
        }

        fn write_provider(&self, name: &str, contents: &str) {
            let path = self
                .scratch
                .join(crate::workspace::PROVIDERS_DIR)
                .join(format!("{name}.toml"));
            fs::write(path, contents).expect("write a fixture provider definition");
        }

        fn workspace(&self) -> Workspace {
            Workspace::new(self.scratch.to_path_buf())
        }
    }

    /// Everything a plan says, in the order it says it — one string two runs can be compared by.
    ///
    /// Deliberately not a hash: when two plans differ this prints the difference, so a
    /// scheduling-ordered artifact is legible in the failure rather than being one hex digest
    /// against another.
    fn fingerprint(plan: &Plan) -> String {
        let mut out = format!("providers: {}\n", plan.providers.join(" "));
        for artifact in &plan.artifacts {
            out.push_str(&format!(
                "--- artifact {} ({:?})\n{}\n",
                artifact.path.display(),
                artifact.change,
                artifact.contents
            ));
        }
        for diagnostic in &plan.diagnostics {
            out.push_str(&format!("--- diagnostic\n{diagnostic}\n"));
        }
        for orphan in &plan.orphans {
            out.push_str(&format!("--- orphan {}\n", orphan.path.display()));
        }
        out
    }

    /// A seeded permutation of `items`, standing in for the order concurrent compiles finish in.
    ///
    /// Deterministic on purpose — a flaky proof that a determinism assertion has teeth would be
    /// worth nothing — and a three-shift xorshift64 rather than a dependency.
    fn seeded_shuffle<T>(mut items: Vec<T>) -> Vec<T> {
        let mut state: u64 = 0xc544_c544_c544_c544;
        for index in (1..items.len()).rev() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            items.swap(index, (state % (index as u64 + 1)) as usize);
        }
        items
    }

    /// The providers a determinism fixture uses, uneven on purpose — see [`definition`].
    const UNEVEN: &[(&str, usize)] = &[
        ("alpha", 12),
        ("bravo", 1),
        ("charlie", 1),
        ("delta", 1),
        ("echo", 1),
        ("foxtrot", 1),
    ];

    /// Acceptance (C-544): "artifact write order, diagnostic/refusal message order, and the
    /// lockfile row order are identical to the sequential build's".
    ///
    /// Width 1 **is** the sequential build — [`compile_all`] spawns no thread there — so this
    /// compares every concurrent plan against exactly what this code produced before the story, on
    /// the same inputs and in the same process.
    #[test]
    fn the_plan_is_identical_at_every_compile_width() {
        let fixture = Fixture::with("pipeline-width", UNEVEN);
        let workspace = fixture.workspace();

        let sequential = fingerprint(
            &plan_at_width(&workspace, None, None, Some(1)).expect("the sequential plan compiles"),
        );

        for width in [2, 3, 6, 64] {
            let concurrent = plan_at_width(&workspace, None, None, Some(width))
                .unwrap_or_else(|error| panic!("the plan at width {width} compiles: {error:#}"));
            assert_eq!(
                sequential,
                fingerprint(&concurrent),
                "compiling at width {width} produced a different plan from the sequential one"
            );
        }
    }

    /// "Output ordering stays defined by content, never by completion order."
    ///
    /// `web/public/catalog.json` is where that is load-bearing rather than incidental:
    /// [`site::document_with_core`] serialises the entries in the order it is handed them and sorts
    /// nothing, so the published provider array *is* the fold order. The plan's artifact list
    /// cannot show this — it is sorted by path — which is why this reads inside the document.
    #[test]
    fn the_published_catalogue_lists_the_providers_in_provider_order() {
        let fixture = Fixture::with("pipeline-order", UNEVEN);
        let workspace = fixture.workspace();

        let plan = plan_at_width(&workspace, None, None, Some(8)).expect("the plan compiles");
        let published = plan
            .artifacts
            .iter()
            .find(|artifact| artifact.path == workspace.site_catalog_path())
            .expect("a whole-catalogue plan writes catalog.json");
        let document: serde_json::Value =
            serde_json::from_str(&published.contents).expect("catalog.json is JSON");
        let ids: Vec<&str> = document["providers"]
            .as_array()
            .expect("catalog.json carries a provider array")
            .iter()
            .map(|provider| {
                provider["id"]
                    .as_str()
                    .expect("every published provider carries an id")
            })
            .collect();

        let expected: Vec<&str> = UNEVEN.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            ids, expected,
            "the published provider array follows the order the compiles finished in"
        );
    }

    /// The seeded ordering hazard, kept as a test so the assertions above cannot quietly lose their
    /// teeth.
    ///
    /// It folds one run's results in a seeded permutation — standing in for the order the providers
    /// happened to finish in — and requires the output to move. If this ever stops holding, [`merge`]
    /// has stopped being what orders the output, and
    /// [`the_plan_is_identical_at_every_compile_width`] would be green on a build whose published
    /// artifacts followed the scheduler.
    #[test]
    fn folding_in_completion_order_would_move_a_published_artifact() {
        let fixture = Fixture::with("pipeline-hazard", UNEVEN);
        let workspace = fixture.workspace();
        let providers = discovery::discover(&workspace, None).expect("the fixture has providers");

        let ordered = merge(
            compile_all(&workspace, &providers, None, true, 1).expect("the providers compile"),
        );
        let shuffled = merge(seeded_shuffle(
            compile_all(&workspace, &providers, None, true, 1).expect("the providers compile"),
        ));

        let paths = |merged: &Merged| {
            merged
                .artifacts
                .iter()
                .map(|artifact| artifact.path.clone())
                .collect::<Vec<_>>()
        };
        assert_ne!(
            paths(&ordered),
            paths(&shuffled),
            "the seeded permutation left the fold order alone, so it proves nothing"
        );
        assert_ne!(
            site::document_with_core(ordered.entries, None).expect("the ordered document renders"),
            site::document_with_core(shuffled.entries, None)
                .expect("the shuffled document renders"),
            "a completion-ordered fold produced the same catalog.json as a provider-ordered one, so \
             the determinism assertions in this module cannot see a scheduling-ordered artifact"
        );
    }

    /// "A refusal raised by one provider must surface identically regardless of scheduling", and
    /// "a provider's compile failure still fails the build loudly with the same message it fails
    /// with today".
    ///
    /// Two providers are broken, so the question has a wrong answer available: a concurrent run
    /// compiles both and could report whichever failed first. They sit behind a twelve-operation
    /// provider, so both failures are raised while `alpha` is still compiling.
    #[test]
    fn a_refusal_is_the_first_one_in_provider_order_at_every_width() {
        let fixture = Fixture::with("pipeline-refusal", &[("alpha", 12), ("zulu", 1)]);
        fixture.write_provider(
            "broken-one",
            "vendor = \"No id Inc.\"\nbase_url = \"https://api.one.example\"\n",
        );
        fixture.write_provider(
            "broken-two",
            "id = \"broken-two\"\nvendor = \"No base URL Inc.\"\n",
        );
        let workspace = fixture.workspace();

        let sequential = format!(
            "{:#}",
            plan_at_width(&workspace, None, None, Some(1))
                .expect_err("a broken provider fails the plan")
        );
        assert!(
            sequential.contains("provider `broken-one`"),
            "the sequential refusal does not name the first broken provider: {sequential}"
        );
        assert!(
            !sequential.contains("broken-two"),
            "the sequential refusal reports past the first failure: {sequential}"
        );

        for width in [2, 4, 64] {
            let concurrent = format!(
                "{:#}",
                plan_at_width(&workspace, None, None, Some(width))
                    .expect_err("a broken provider fails the plan")
            );
            assert_eq!(
                sequential, concurrent,
                "the refusal at width {width} is not the one a sequential build raises"
            );
        }
    }
}
