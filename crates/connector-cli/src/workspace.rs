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

/// Generated, committed artifacts: `connectors/<name>.flux` and `<name>.connector.toml`.
pub const ARTIFACTS_DIR: &str = "connectors";

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
    pub fn artifacts_dir(&self) -> PathBuf {
        self.root.join(ARTIFACTS_DIR)
    }

    /// `<root>/connectors/<provider>.flux`.
    pub fn module_path(&self, provider: &str) -> PathBuf {
        self.artifacts_dir()
            .join(format!("{provider}.{MODULE_EXT}"))
    }

    /// `<root>/connectors/<provider>.connector.toml`.
    pub fn manifest_path(&self, provider: &str) -> PathBuf {
        self.artifacts_dir()
            .join(format!("{provider}.{MANIFEST_SUFFIX}"))
    }

    /// `<root>/crates/catalog`.
    pub fn catalog_dir(&self) -> PathBuf {
        self.root.join(CATALOG_DIR)
    }

    /// `<root>/crates/catalog/ops/<provider>` — that provider's per-operation renderings.
    ///
    /// One directory per provider, not one flat directory: 25 operations ship today and a
    /// spec-ingested babelforce alone offers 163, so the count grows linearly with selection. The
    /// provider level is the split available now; C-37's `gid` — a versioned resource group such as
    /// `com.babelforce.api/manager/calls:v1` — is the natural second level, and it slots in below
    /// this one without moving anything above it.
    pub fn catalog_ops_dir(&self, provider: &str) -> PathBuf {
        self.catalog_dir().join("ops").join(provider)
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
        self.catalog_dir()
            .join("src")
            .join("generated")
            .join(format!("{provider}.rs"))
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

    /// `path` relative to the root when it is below it, for stable, machine-independent output.
    pub fn display_path<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(&self.root).unwrap_or(path)
    }
}
