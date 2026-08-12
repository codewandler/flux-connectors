//! Where a build reads its inputs and writes its artifacts.
//!
//! Every path a command touches is derived here from a single root, so a test can point a whole
//! build at a fixture tree and the production layout is the same code path.

use std::path::{Path, PathBuf};

/// Committed provider definitions: `providers/<name>.toml`.
pub const PROVIDERS_DIR: &str = "providers";

/// The vendored spec cache: `specs/<name>/<version>.json`.
///
/// Committed deliberately — it is what makes a build hermetic, offline and reviewable years later.
pub const SPECS_DIR: &str = "specs";

/// How a vendored spec spells itself in a provider file: `specs/<provider>/<file>`.
///
/// Derived from the provider name and the file name rather than by stripping a workspace root, so it
/// is the same string no matter where the repository is checked out — which matters because it is
/// compared for equality against `[spec] path`, a value an author typed by hand (C-4).
pub fn spec_path(provider: &str, file: &Path) -> String {
    format!(
        "{SPECS_DIR}/{provider}/{}",
        file.file_name().unwrap_or_default().to_string_lossy()
    )
}

/// Generated, committed artifacts: `connectors/<name>.flux` and `<name>.connector.toml`.
pub const ARTIFACTS_DIR: &str = "connectors";

/// The canonical per-provider catalog documents: `catalog/<name>.catalog.json` (C-536).
///
/// At the repository root beside `connectors/`, as the catalog-artifact design's diagram places
/// them: the reviewed artifact of Decision 0022, one deterministic JSON document per provider.
pub const DOCUMENTS_DIR: &str = "catalog";

/// The suffix a canonical document's file name carries: `zendesk.catalog.json`.
pub const DOCUMENT_SUFFIX: &str = "catalog.json";

/// The committed JSON Schema the documents validate against, beside them under [`DOCUMENTS_DIR`].
pub const DOCUMENT_SCHEMA_FILE: &str = "connector-document.schema.json";

/// The `.flux` module extension.
pub const MODULE_EXT: &str = "flux";

/// The manifest suffix. Not an extension — `zendesk.connector.toml` has stem `zendesk.connector`.
pub const MANIFEST_SUFFIX: &str = "connector.toml";

/// The catalog crate (C-38), whose generated half a build writes.
///
/// The renderings live **inside** the crate rather than beside `connectors/`, because the crate
/// embeds them with `include_str!` and a path that escapes the package root is one `cargo package`
/// would not carry — a catalog that compiled here and nowhere else.
pub const CATALOG_DIR: &str = "crates/catalog";

/// The public site's data directory (C-42), holding the generated `catalog.json`.
///
/// Outside `connectors/` deliberately: that directory holds what a user *installs* into
/// `~/.flux/flows`, and a JSON document a website fetches is not that.
///
/// It is VitePress's `public/` directory (C-44), which is served verbatim at the site root — so the
/// explorer fetches `/flux-connectors/catalog.json` with no copy step and no build plumbing between
/// the Rust pipeline and the Node one. A sibling directory at the repository root was the original
/// choice; it meant two top-level directories for one website, and a copy step that could ship a
/// stale document. This pipeline still owns the file; the site merely reads it.
pub const SITE_DIR: &str = "web/public";

/// The site's generated catalogue: `web/public/catalog.json`.
pub const SITE_CATALOG: &str = "catalog.json";

/// The checked Flux-owned core catalogue snapshot.
pub const CORE_CATALOG_SOURCE: &str = "specs/flux/core-v1.json";

/// The repository's documentation images: `assets/`.
pub const ASSETS_DIR: &str = "assets";

/// The stem shared by the README snippet and everything rendered from it (C-45).
pub const SNIPPET_STEM: &str = "readme-snippet";

/// The file stem a service's artifacts share: `zendesk` for the `default` service, `aws-s3` for a
/// named one.
fn artifact_stem(provider: &str, service: &str) -> String {
    if service == connector_spec::DEFAULT_SERVICE {
        provider.to_owned()
    } else {
        format!("{provider}-{service}")
    }
}

/// A repository root plus the layout convention applied to it.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Treat `root` as a flux-connectors repository.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The repository root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<root>/providers`.
    pub fn providers_dir(&self) -> PathBuf {
        self.root.join(PROVIDERS_DIR)
    }

    /// `<root>/specs/<provider>`.
    pub fn spec_dir(&self, provider: &str) -> PathBuf {
        self.root.join(SPECS_DIR).join(provider)
    }

    /// `<root>/connectors`.
    ///
    /// The **artifact root** of the installable-unit family (C-429): every file in it is a module or
    /// a manifest a build wrote, so one no plan claims is a unit whose service stopped existing.
    pub fn artifacts_dir(&self) -> PathBuf {
        self.root.join(ARTIFACTS_DIR)
    }

    /// `<root>/catalog`.
    ///
    /// The **artifact root** of the canonical-document family (C-536, C-429): every `.json` in it
    /// is a per-provider document or their shared schema, all written by a build, so one no plan
    /// claims is a document whose provider stopped existing.
    pub fn documents_dir(&self) -> PathBuf {
        self.root.join(DOCUMENTS_DIR)
    }

    /// `<root>/catalog/<provider>.catalog.json` — the canonical per-provider document (C-536).
    pub fn document_path(&self, provider: &str) -> PathBuf {
        self.documents_dir()
            .join(format!("{provider}.{DOCUMENT_SUFFIX}"))
    }

    /// `<root>/catalog/connector-document.schema.json` — the schema the documents validate against.
    pub fn document_schema_path(&self) -> PathBuf {
        self.documents_dir().join(DOCUMENT_SCHEMA_FILE)
    }

    /// `<root>/connectors/<provider>.flux` — the module of a provider's `default` service.
    pub fn module_path(&self, provider: &str) -> PathBuf {
        self.service_module_path(provider, connector_spec::DEFAULT_SERVICE)
    }

    /// `<root>/connectors/<provider>.connector.toml` — the manifest of the `default` service.
    pub fn manifest_path(&self, provider: &str) -> PathBuf {
        self.service_manifest_path(provider, connector_spec::DEFAULT_SERVICE)
    }

    /// `<root>/connectors/<provider>-<service>.flux`, or `<provider>.flux` for the `default` service.
    ///
    /// The emitted unit is the service (C-49), and the reserved `default` service is elided from the
    /// file name for the same reason it is elided from an address: it is an internal name for "this
    /// provider has one API surface", and a `zendesk-default.flux` would publish it.
    pub fn service_module_path(&self, provider: &str, service: &str) -> PathBuf {
        self.artifacts_dir()
            .join(format!("{}.{MODULE_EXT}", artifact_stem(provider, service)))
    }

    /// `<root>/connectors/<provider>-<service>.connector.toml`, eliding `default` as above.
    pub fn service_manifest_path(&self, provider: &str, service: &str) -> PathBuf {
        self.artifacts_dir().join(format!(
            "{}.{MANIFEST_SUFFIX}",
            artifact_stem(provider, service)
        ))
    }

    /// `<root>/crates/catalog`.
    pub fn catalog_dir(&self) -> PathBuf {
        self.root.join(CATALOG_DIR)
    }

    /// `<root>/crates/catalog/ops` — every provider's renderings, one directory each.
    ///
    /// The **artifact root** of the rendering family (C-429): a `.flux` file anywhere below here is
    /// a rendering, and one no plan claims is an orphan. Rooted at the parent rather than at
    /// [`Self::catalog_ops_dir`] on purpose — a provider removed from `providers/` altogether takes
    /// its own directory out of the plan, and a root that only existed while the provider did would
    /// be exactly the class of orphan nobody finds.
    pub fn catalog_ops_root(&self) -> PathBuf {
        self.catalog_dir().join("ops")
    }

    /// `<root>/crates/catalog/ops/<provider>` — that provider's per-operation renderings.
    ///
    /// One directory per provider, not one flat directory: 25 operations ship today and a
    /// spec-ingested babelforce alone offers 163, so the count grows linearly with selection. The
    /// provider level is the split available now; C-37's `gid` — a versioned resource group such as
    /// `com.babelforce.api/manager/calls:v1` — is the natural second level, and it slots in below
    /// this one without moving anything above it.
    pub fn catalog_ops_dir(&self, provider: &str) -> PathBuf {
        self.catalog_ops_root().join(provider)
    }

    /// `<root>/crates/catalog/src/generated` — one generated table per provider.
    ///
    /// The **artifact root** of that family (C-429). Its parent, `crates/catalog/src`, is not one:
    /// it holds `lib.rs` and the hand-written half of the crate, and the one generated file directly
    /// in it — [`Self::catalog_index_path`] — is a single file a full run always writes and so
    /// cannot be orphaned.
    pub fn catalog_generated_dir(&self) -> PathBuf {
        self.catalog_dir().join("src").join("generated")
    }

    /// `<root>/crates/catalog/ops/<provider>/<operation>.flux`.
    pub fn catalog_op_path(&self, provider: &str, operation: &str) -> PathBuf {
        self.catalog_ops_dir(provider)
            .join(format!("{operation}.{MODULE_EXT}"))
    }

    /// `<root>/crates/catalog/src/generated/<provider>.rs` — the generated table for one provider.
    ///
    /// Per provider, not one file for all of them, so that `build --provider zendesk` regenerates
    /// exactly what it compiled. A single index would have to drop the providers the run did not
    /// look at.
    pub fn catalog_module_path(&self, provider: &str) -> PathBuf {
        self.catalog_generated_dir().join(format!("{provider}.rs"))
    }

    /// `<root>/crates/catalog/src/generated.rs` — the index naming every provider's module (C-104).
    ///
    /// The whole-catalogue counterpart to [`Self::catalog_module_path`], and it carries the same
    /// consequence [`Self::site_catalog_path`] does: it is a function of a **full** run, so
    /// [`crate::pipeline::plan_selected`] emits it only when nothing narrowed the run. It was
    /// hand-maintained until C-104 for exactly that reason, which made it the single file every
    /// provider story had to append to — and therefore the reason two of them could never run at
    /// once.
    pub fn catalog_index_path(&self) -> PathBuf {
        self.catalog_dir().join("src").join("generated.rs")
    }

    /// `<root>/web/public/catalog.json` — the whole catalogue as one JSON document (C-42).
    ///
    /// One file for every provider, not one per provider: a website wants one fetch, and the
    /// explorer's filters are queries across the whole catalogue. The cost is that it is not a
    /// function of a `--provider` run, which is why [`crate::pipeline::plan`] emits it only for a
    /// full build. See [`crate::site`].
    pub fn site_catalog_path(&self) -> PathBuf {
        self.root.join(SITE_DIR).join(SITE_CATALOG)
    }

    /// `<root>/connectors.lock` — the drift record for the whole catalogue (C-7, written by C-189).
    ///
    /// At the repository root rather than under `connectors/`, and the name is
    /// [`connector_spec::LOCKFILE_NAME`] rather than a literal here, so the writer and
    /// `flux-connectors check` (C-14) cannot disagree about which file they mean. The root is where
    /// a reader looks for a lockfile — it is a property of the repository, not of one directory of
    /// artifacts — and it is the same place `Cargo.lock` sits.
    ///
    /// **Whole-catalogue**: it holds one row per provider, so only a full run can write it. See
    /// [`crate::pipeline::plan_selected`].
    pub fn lockfile_path(&self) -> PathBuf {
        self.root.join(connector_spec::LOCKFILE_NAME)
    }

    /// `path` as `connectors.lock` keys it: repository-relative, `/`-separated on every platform.
    ///
    /// [`Self::display_path`] is the human-facing form and renders with the host's separator, which
    /// is fine for a message and wrong for a hashed artifact — a lockfile built on Windows would
    /// not be byte-identical to one built here, and every key would read as drift.
    pub fn artifact_key(&self, path: &Path) -> String {
        self.display_path(path)
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    }

    /// `<root>/specs/flux/core-v1.json` — the vendored output of
    /// `flux catalog core --format json`.
    pub fn core_catalog_source_path(&self) -> PathBuf {
        self.root.join(CORE_CATALOG_SOURCE)
    }

    /// A generated public resource below `web/public/`.
    pub fn site_public_path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.root.join(SITE_DIR).join(relative)
    }

    /// `<root>/assets/readme-snippet.flux` — the Flux the README shows (C-45).
    ///
    /// A committed input, not an artifact: it is one operation lifted verbatim out of
    /// `connectors/zendesk.flux`, and `tests/readme_snippet.rs` holds it to that.
    pub fn snippet_path(&self) -> PathBuf {
        self.root
            .join(ASSETS_DIR)
            .join(format!("{SNIPPET_STEM}.{MODULE_EXT}"))
    }

    /// `<root>/assets/readme-snippet-<theme>.svg` — one palette's rendering of the snippet.
    ///
    /// The names are load-bearing: README.md selects between them with `<picture>` and
    /// `prefers-color-scheme`, so renaming one silently breaks dark mode for every reader.
    pub fn snippet_svg_path(&self, theme: &str) -> PathBuf {
        self.root
            .join(ASSETS_DIR)
            .join(format!("{SNIPPET_STEM}-{theme}.svg"))
    }

    /// `<root>/assets/readme-snippet.png` — the convenience raster (C-45), written only on request.
    pub fn snippet_png_path(&self) -> PathBuf {
        self.root
            .join(ASSETS_DIR)
            .join(format!("{SNIPPET_STEM}.png"))
    }

    /// `path` relative to the root when it is below it, for stable, machine-independent output.
    pub fn display_path<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(&self.root).unwrap_or(path)
    }
}
