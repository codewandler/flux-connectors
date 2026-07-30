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

    /// `path` relative to the root when it is below it, for stable, machine-independent output.
    pub fn display_path<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(&self.root).unwrap_or(path)
    }
}
