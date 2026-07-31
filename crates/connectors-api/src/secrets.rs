//! Which credential store this host binds, and where it puts it.
//!
//! `connector-secrets` owns the stores; this module owns the **choice between them**, because the
//! choice is the part that is a deployment decision rather than a library one. The port has always
//! been `Arc<dyn SecretStore>` — swapping the implementation was never the hard part. What was
//! missing (C-207) is the choice being expressible, its default being safe, and a bad value being a
//! startup error rather than a host that quietly forgets.
//!
//! # The one rule
//!
//! **There is no silent fallback to memory.** Every path below either binds the store it was asked
//! for or returns an error that stops the process. A host that answered "the store would not open,
//! so I will hold your credentials in memory instead" would forget them on the next restart while
//! looking exactly like one that had not — which is the failure this module exists to end, arrived
//! at by a different road.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use connector_pack::{FileStore, MemoryStore, SecretStore};

/// The variable that selects the store.
pub const STORE_ENV: &str = "CONNECTORS_CREDENTIAL_STORE";

/// The directory, under an operator's data home, this host keeps its own state in.
const DATA_DIRECTORY: &str = "connectors-api";

/// The store file's name inside that directory.
const STORE_FILE: &str = "credentials";

/// Where this host's credentials are kept.
///
/// Deliberately not `Copy` or `Default`: choosing where a plaintext credential lives should be a
/// value somebody constructed, not one that appears because a field was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreChoice {
    /// In process memory. Stopping the host is the cleanup.
    ///
    /// Still the right answer for a test, for an embedder that has its own store, and for an
    /// operator who has decided this machine is not one they will leave a vendor token on. It is
    /// **not** a fallback — nothing selects it except a caller or an operator asking for it by
    /// name.
    Memory,
    /// In one `0600` file at this path, which survives the process.
    File(PathBuf),
}

impl StoreChoice {
    /// Read the choice from the environment.
    ///
    /// `Ok(None)` means the operator expressed no preference, which is a different fact from
    /// "memory" and is left to the caller to resolve — [`crate::App::new`] and
    /// [`crate::App::deployed`] resolve it differently and both say why.
    ///
    /// # Errors
    ///
    /// Any value this cannot parse. Loudly, at startup, because the alternative is a host that
    /// starts against a store the operator did not ask for.
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        match std::env::var(STORE_ENV) {
            Ok(spec) if spec.trim().is_empty() => Ok(None),
            Ok(spec) => Ok(Some(Self::parse(spec.trim())?)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => {
                anyhow::bail!("{STORE_ENV} is set to something that is not valid UTF-8")
            }
        }
    }

    /// Parse one spec: `memory`, `file`, or `file:<absolute path>`.
    ///
    /// # Errors
    ///
    /// Anything else, and a `file:` with a relative path — a relative store path means the
    /// credentials move when the operator starts the host from another directory, which is a way of
    /// forgetting them that looks like a fresh install.
    pub fn parse(spec: &str) -> anyhow::Result<Self> {
        match spec {
            "memory" => Ok(Self::Memory),
            "file" => Ok(Self::File(default_path()?)),
            _ => match spec.strip_prefix("file:") {
                Some("") => anyhow::bail!(
                    "{STORE_ENV}=`file:` names no path. Use `file` for the default location \
                     ({}), or `file:/an/absolute/path`.",
                    default_path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(
                            |_| "unavailable: neither XDG_DATA_HOME nor HOME is set".to_owned()
                        )
                ),
                Some(path) if !Path::new(path).is_absolute() => anyhow::bail!(
                    "{STORE_ENV}=`file:{path}` is a relative path. A credential store must be \
                     named absolutely, or it moves with the directory the host was started from \
                     and an operator's credentials appear to have been lost."
                ),
                Some(path) => Ok(Self::File(PathBuf::from(path))),
                None => anyhow::bail!(
                    "{STORE_ENV}={spec:?} is not a store this host has. It is one of:\n  \
                     `file`                    — one 0600 file at {}\n  \
                     `file:/an/absolute/path`  — the same, somewhere you chose\n  \
                     `memory`                  — nothing survives the process, which is the old \
                     behaviour and is now something you ask for",
                    default_path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|_| "$XDG_DATA_HOME/connectors-api/credentials".to_owned())
                ),
            },
        }
    }

    /// Bind it.
    ///
    /// # Errors
    ///
    /// Whatever the store says. **Never falls back** — see the module documentation.
    pub fn open(&self) -> anyhow::Result<Arc<dyn SecretStore>> {
        match self {
            Self::Memory => Ok(Arc::new(MemoryStore::new())),
            Self::File(path) => {
                let store = FileStore::open(path).map_err(|error| {
                    anyhow::anyhow!(
                        "the credential store at {} could not be opened: {error}\n\
                         This host does not fall back to holding credentials in memory — that \
                         would start successfully and forget everything on the next restart. Fix \
                         the path or set {STORE_ENV}=memory deliberately.",
                        path.display()
                    )
                })?;
                Ok(Arc::new(store))
            }
        }
    }

    /// The file this choice keeps credentials in, if it keeps them in one.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Memory => None,
            Self::File(path) => Some(path),
        }
    }

    /// **What an operator is told at startup**, in full sentences and without a value in it.
    ///
    /// This is the banner text, and it lives here rather than in `main.rs` for one reason: the
    /// sentence *"credentials are held in memory only: stopping the process is the cleanup"* was
    /// true of every build of this host until C-207 and is false of some of them now. A banner
    /// assembled next to the store it describes cannot drift from it; one written out by hand in
    /// the binary already had.
    ///
    /// It states what protects the credentials **and what does not**, because a store that lands a
    /// vendor token on disk in recoverable form is a new exposure and the operator is the only
    /// person who can weigh it.
    pub fn banner(&self) -> String {
        match self {
            Self::Memory => format!(
                "Credentials are held in memory only: stopping the process is the cleanup,\n\
                 and every connector must be wired again after a restart.\n\
                 Set {STORE_ENV}=file to keep them in a 0600 file instead."
            ),
            Self::File(path) => format!(
                "Credentials are kept in {} and SURVIVE A RESTART.\n\
                 They are NOT ENCRYPTED. A 0600 file mode inside a 0700 directory is the whole\n\
                 of what protects them — anyone who can read that file, and any backup that\n\
                 copies it, has your vendor tokens.\n\
                 To point them elsewhere: {STORE_ENV}=file:/an/absolute/path\n\
                 To hold nothing at rest:  {STORE_ENV}=memory\n\
                 To destroy them:          rm -r {}",
                path.display(),
                // The **directory**, not the file, and the difference is not tidiness. A write
                // interrupted by a crash can leave a `0600` temporary beside the store holding a
                // complete copy of every credential; the next `open` reaps it, but an operator who
                // deletes only the file and never starts the host again has revoked nothing. `rm -r`
                // on the directory is the instruction that is true in every case.
                path.parent().unwrap_or(path).display()
            ),
        }
    }
}

/// Where a file store goes when the operator did not say.
///
/// `$XDG_DATA_HOME/connectors-api/credentials`, falling back to `$HOME/.local/share/...` — the
/// XDG base directory default, spelled out rather than pulled in as a dependency for two lines.
///
/// **It is an operator's data home, and never a directory this host serves or builds from.** The
/// crate root is what `App::new` is handed (`CARGO_MANIFEST_DIR` in the tests, the current
/// directory in the binary), so a store defaulting relative to *that* would sit inside a
/// repository checkout — one `git add -A` from being committed, and inside the tree the host's own
/// `System` workspace is rooted at. [`refuse_inside`] asserts the separation rather than trusting
/// this comment.
///
/// # Errors
///
/// When neither `XDG_DATA_HOME` nor `HOME` is set, which is a real state in a container and is
/// better reported than guessed at.
pub fn default_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".local").join("share"))
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "neither XDG_DATA_HOME nor HOME names an absolute directory, so there is no \
                 default location for a credential store. Set {STORE_ENV}=file:/an/absolute/path."
            )
        })?;
    Ok(home.join(DATA_DIRECTORY).join(STORE_FILE))
}

/// Refuse a store that would sit inside `root`.
///
/// `root` is the workspace the host dispatches under — in the binary, the directory it was started
/// from; in a test, the crate. Two distinct reasons, either sufficient:
///
/// - **It is a directory the host is rooted at.** `flux_system::Workspace` is constructed over it,
///   and a credential file inside a tree a tool can be pointed at is a credential file one path
///   traversal from being read out. Nothing here serves static files from `root` today, and that is
///   a fact about the current route table rather than a property of the type.
/// - **It is, in practice, a repository checkout.** `cargo run -p connectors-api` from the
///   repository root is how every reader of the README starts this host, and a `credentials` file
///   under it is one `git add -A` from a commit. That has happened in this repository before, with
///   a stray capture (`3e86413`).
///
/// # Errors
///
/// When `store` is `root` or below it. The message names both paths; neither is a secret.
pub fn refuse_inside(root: &Path, store: &Path) -> anyhow::Result<()> {
    // Compared after canonicalising as far as each path exists, so `/repo/./x` and a symlinked
    // `root` do not read as separate trees. A path that does not exist yet — the usual case for the
    // store file — falls back to its own lexical form, which is why the store's *parent* chain is
    // walked rather than the file itself.
    let root = settle(root);
    let store = settle(store);
    if store.starts_with(&root) {
        anyhow::bail!(
            "the credential store {} is inside {}, which is the directory this host runs under. \
             A plaintext credential file there is one `git add -A` from being committed and sits \
             inside the tree the host's own workspace is rooted at. Point {STORE_ENV} somewhere \
             outside it.",
            store.display(),
            root.display()
        );
    }
    Ok(())
}

/// The longest existing prefix of `path`, canonicalised, with the rest appended.
fn settle(path: &Path) -> PathBuf {
    let mut remainder = Vec::new();
    let mut cursor = path;
    loop {
        if let Ok(resolved) = cursor.canonicalize() {
            let mut settled = resolved;
            for component in remainder.iter().rev() {
                settled.push(component);
            }
            return settled;
        }
        let Some(parent) = cursor.parent() else {
            return path.to_path_buf();
        };
        match cursor.file_name() {
            Some(name) => remainder.push(name.to_owned()),
            None => return path.to_path_buf(),
        }
        cursor = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_spellings_parse() {
        assert_eq!(
            StoreChoice::parse("memory").expect("memory"),
            StoreChoice::Memory
        );
        assert_eq!(
            StoreChoice::parse("file:/var/lib/connectors/credentials").expect("an absolute file"),
            StoreChoice::File(PathBuf::from("/var/lib/connectors/credentials"))
        );
        assert!(matches!(
            StoreChoice::parse("file"),
            Ok(StoreChoice::File(_)) | Err(_)
        ));
    }

    /// **A bad value stops the host**, and the refusal lists what would have worked. A store
    /// selection that fell through to memory is the whole failure C-207 exists to end.
    #[test]
    fn a_bad_value_is_refused_and_never_becomes_memory() {
        for spec in [
            "vault",
            "File",
            "",
            "file:",
            "file:relative/path",
            "sqlite:///x",
            "yes",
        ] {
            let refusal = StoreChoice::parse(spec)
                .err()
                .unwrap_or_else(|| panic!("{spec:?} was accepted as a store"));
            let message = refusal.to_string();
            assert!(
                message.contains(STORE_ENV),
                "{spec:?}: the refusal must name the variable: {message}"
            );
        }
    }

    /// The default lands under an operator's data home, not under the tree the host runs from.
    #[test]
    fn the_default_path_is_under_a_data_home() {
        let Ok(path) = default_path() else {
            // Neither variable set; the error path is the assertion above.
            return;
        };
        assert!(path.is_absolute(), "{}", path.display());
        assert!(path.ends_with(Path::new(DATA_DIRECTORY).join(STORE_FILE)));
        assert!(
            !path.starts_with(env!("CARGO_MANIFEST_DIR")),
            "the default store {} is inside the crate this host is built from",
            path.display()
        );
    }

    /// The check that makes the paragraph above an assertion rather than a hope.
    #[test]
    fn a_store_inside_the_root_is_refused() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for inside in [
            root.join("credentials"),
            root.join("src").join("credentials"),
            root.join(".").join("nested").join("credentials"),
            root.to_path_buf(),
        ] {
            assert!(
                refuse_inside(root, &inside).is_err(),
                "{} was accepted, and it is inside {}",
                inside.display(),
                root.display()
            );
        }
        assert!(refuse_inside(root, Path::new("/var/lib/x/credentials")).is_ok());
        // A sibling whose name merely starts with the root's is not inside it.
        assert!(refuse_inside(Path::new("/a/root"), Path::new("/a/rootless/credentials")).is_ok());
    }

    /// The banner is what an operator reads, so it must not overstate the file store.
    #[test]
    fn the_banner_tells_the_truth_about_both_stores() {
        let memory = StoreChoice::Memory.banner();
        assert!(memory.contains("memory only"));
        assert!(memory.contains("stopping the process is the cleanup"));

        let file = StoreChoice::File(PathBuf::from("/var/lib/connectors/credentials")).banner();
        assert!(file.contains("SURVIVE A RESTART"));
        assert!(
            file.contains("NOT ENCRYPTED"),
            "the banner must not imply a protection the store does not have: {file}"
        );
        assert!(
            // The **directory**. A crashed write can leave a temporary holding a full copy beside
            // the store, so `rm` on the file alone is a revoke that does not revoke.
            file.contains("rm -r /var/lib/connectors"),
            "a credential store with no documented delete is one an operator cannot revoke: {file}"
        );
        assert!(
            file.contains(STORE_ENV),
            "the banner must say how to move it"
        );
    }
}
