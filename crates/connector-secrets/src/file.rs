//! A [`SecretStore`] that survives the process in one platform-protected file.
//!
//! [`MemoryStore`](crate::MemoryStore) is honest about what it is — *"the process exiting is the
//! cleanup"* — and that is the right store for a test and the wrong one for a deployment. A host
//! with durable accounts whose credentials evaporate on restart makes an operator re-paste every
//! token every time, which is the habit that gets a token pasted somewhere it should not be. This is
//! the other end: a single file, owned by one operator, that a restart does not empty.
//!
//! # What protects a credential here, stated plainly
//!
//! **Nothing cryptographic. The values in this file are recoverable by anyone who can read it.**
//!
//! | | |
//! |---|---|
//! | Unix owner plus file mode `0600` and directory mode `0700` | set in the create calls, not fixed up afterwards; owner, mode, kind and no-follow handle metadata are checked before every read or write |
//! | Windows process `TokenUser` SID plus a non-null protected DACL | set in the create calls with one explicit full-control allow entry for that SID; owner, DACL, kind and non-reparse handle metadata are checked before every read or write |
//! | the write is atomic | a bounded full rewrite into a protected sibling temporary, flush, then the platform replacement primitive — a failed write leaves the previous file whole |
//! | the file never sits inside a served directory | the path is the caller's, and [`connectors_api`](https://docs.rs/) refuses one under its own workspace root |
//! | hex encoding | **framing, not protection.** It exists so a value containing a newline cannot forge a second entry, and so a careless `grep` over the filesystem does not match a token. `xxd -r` undoes it. |
//! | encryption at rest | **absent.** There is no key, no passphrase and no OS keychain integration. |
//! | protection from Unix root, Windows administrators, or a backup that copies the file | **absent.** |
//!
//! That table is the whole security argument, and it is deliberately short. A store that implied more
//! would be worse than one that implies nothing: the operator's own decision — whether this machine
//! is one they are willing to leave a vendor token on — is only theirs to make if it is stated.
//! `VaultStore` remains the answer for a deployment that is not one operator's laptop.
//!
//! # Concurrency
//!
//! One active 0.20 writer. [`FileStore::open`] takes a non-blocking exclusive kernel lease before it
//! reads, recovers or cleans state and holds the lease for the store lifetime. Released 0.19.1 did
//! not take this lease, so every legacy writer must be stopped before the first 0.20 open; concurrent
//! mixed-version writing is unsupported. A store meant for several writers wants a real backend,
//! which is what the [`SecretStore`] port is for.
//!
//! # The formats
//!
//! ```text
//! # codewandler-connector-secrets file store, v1
//! tenants/dev-local/com.anthropic.api/api_key 53454e54494e454c
//! tenants/dev-local/com.zendesk.api/support/api_token 53454e54494e454c
//! ```
//!
//! Clean v1 remains byte-identical until the first prepared-transaction operation. V2 adds one
//! inclusive retired-generation fence and a canonical bounded transaction ledger ahead of the same
//! credential-entry grammar. A fixed owner-only sibling stage carries the complete next image while
//! one transaction is prepared. A fresh 0.19.1 reader refuses v2 by its existing version check.
//!
//! One entry per line, `<address> <hex of the value>`, sorted. A blank line and a `#` comment are
//! skipped. The separator is a space because **no address can contain one** — every segment of a
//! [`CredentialRef`] is validated, so the split is unambiguous rather than merely conventional.
//!
//! Each address is parsed back through the store's [`Layout`] on load and re-rendered, so a line
//! that is not the canonical spelling of the address it names is a loud error rather than a second
//! entry for one credential. That is the `parse(render(r)) == r` law in [`Layout`]'s contract,
//! enforced at the one place where a hand-edited file could break it.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use fs2::FileExt as _;

use crate::{
    batch, CredentialRef, CredentialScope, Layout, Secret, SecretBatch, SecretStore, StoreError,
    TenantLayout,
};

#[cfg(unix)]
#[path = "file/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "file/windows.rs"]
mod platform;
mod prepared;

#[cfg(not(any(unix, windows)))]
compile_error!("FileStore needs a platform-native owner-only filesystem implementation");

/// The first line of every file this store writes.
const HEADER: &str = "# codewandler-connector-secrets file store, v1";

/// The prefix a version line begins with, and the version this store speaks.
///
/// Split out from [`HEADER`] so the version is **checked** rather than merely written. A header a
/// reader skips as a comment is decoration: a future `v2` — one that encrypted the values, say, or
/// changed the separator — would be loaded as `v1`, and the failure would be a wrong answer rather
/// than a refusal. `v2` bytes read as `v1` is exactly the case a credential store must not guess at.
const VERSION_PREFIX: &str = "# codewandler-connector-secrets file store, v";
const VERSION: &str = "1";

/// Maximum encoded bytes accepted from or written to one durable store.
///
/// The entire v1 store is intentionally held in memory for atomic whole-file replacement, so a
/// bound is part of the format's operational contract rather than a transport detail.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;

/// Maximum number of credential entries accepted in one durable store.
pub const MAX_ENTRIES: usize = 4096;

/// Maximum UTF-8 bytes accepted for one credential value.
pub const MAX_VALUE_BYTES: usize = 64 * 1024;

#[cfg(unix)]
/// The mode a store file is created with, and the widest mode one may be opened at.
pub const FILE_MODE: u32 = 0o600;

#[cfg(unix)]
/// The mode the containing directory is created with, and the widest it may be opened at.
pub const DIR_MODE: u32 = 0o700;

/// Distinguishes one process's in-flight temporary files from another's.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// A [`SecretStore`] kept in one file.
///
/// See the [module documentation](self) for what protects the values in it and what does not — the
/// short answer is the current OS identity plus native owner-only filesystem controls, and nothing
/// cryptographic.
///
pub struct FileStore<L = TenantLayout> {
    layout: L,
    path: PathBuf,
    // The whole store, held in memory and written through on every mutation. `BTreeMap` so the file
    // is sorted and a diff between two versions of it is readable — and so `paths()` is ordered, the
    // same property `MemoryStore` offers.
    entries: Mutex<BTreeMap<String, Secret>>,
    transactions: Mutex<prepared::FileTransactions>,
    // The kernel releases this non-blocking exclusive lease if the process exits abruptly.
    _lease: File,
    #[cfg(test)]
    fail_next_write: std::sync::atomic::AtomicBool,
}

impl<L> Drop for FileStore<L> {
    fn drop(&mut self) {
        // Closing the descriptor releases a kernel lock, including after abrupt process exit. The
        // explicit unlock makes ordinary in-process reopen deterministic before field destruction
        // completes and is deliberately best effort during Drop.
        let _ = fs2::FileExt::unlock(&self._lease);
    }
}

impl FileStore<TenantLayout> {
    /// Open — or create — the store at `path`, using the blessed [`TenantLayout`].
    ///
    /// On Unix the containing directory is created at `0700` and the file at `0600`. On Windows
    /// both are created for the process `TokenUser` SID with a protected DACL allowing only that
    /// SID. Existing state with a foreign owner, wider access, a link/reparse point, the wrong kind
    /// or uninspectable metadata is **refused**, never tightened or repaired: it may already have
    /// exposed values, and changing it silently would hide that evidence.
    ///
    /// # Errors
    ///
    /// [`StoreError::Unreachable`] for an IO or security-inspection failure,
    /// [`StoreError::Denied`] for native protection this store will not use, and
    /// [`StoreError::Backend`] for a file it cannot parse. Every one names the filesystem path and
    /// none carries a value.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, StoreError> {
        Self::open_with_layout(path, TenantLayout)
    }
}

impl<L: Layout> FileStore<L> {
    /// Open — or create — the store at `path`, rendering addresses through `layout`.
    ///
    /// # Errors
    ///
    /// As [`FileStore::open`].
    pub fn open_with_layout(path: impl Into<PathBuf>, layout: L) -> Result<Self, StoreError> {
        let path = path.into();

        if let Some(directory) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            platform::ensure_directory(directory)?;
        }

        let lease_path = fixed_sibling(&path, "lease");
        let lease = platform::open_lease(&lease_path, &path)?;
        lease
            .try_lock_exclusive()
            .map_err(|_| StoreError::Conflict {
                path: path.display().to_string(),
                reason: "another FileStore holds the writer/recovery lease".to_owned(),
            })?;

        let (entries, mut transactions, existed) = match platform::open_existing(&path)? {
            Some(mut file) => {
                let contents = read_bounded(&mut file, &path)?;
                if contents.lines().next() == Some(prepared::HEADER_V2) {
                    let (entries, transactions) = prepared::parse_v2(&contents, &layout, &path)?;
                    (entries, transactions, true)
                } else {
                    (
                        parse(&contents, &layout, &path)?,
                        prepared::FileTransactions::default(),
                        true,
                    )
                }
            }
            None => (
                BTreeMap::new(),
                prepared::FileTransactions::default(),
                false,
            ),
        };

        recover_stage(&path, &layout, &mut transactions)?;

        let store = Self {
            layout,
            path,
            entries: Mutex::new(entries),
            transactions: Mutex::new(transactions),
            _lease: lease,
            #[cfg(test)]
            fail_next_write: std::sync::atomic::AtomicBool::new(false),
        };

        // Written eagerly so that "the store exists, with the right mode" is true after `open`
        // rather than after the first `put`. An operator who is told where their credentials live
        // should find something there, and a test asserting the mode should not have to store a
        // credential first to have something to assert about.
        if !existed {
            store.write_through(&store.locked())?;
        }

        store.reap_temporaries();

        Ok(store)
    }

    /// Remove any temporary left behind by a write that did not finish.
    ///
    /// **A crash between the write and the `rename(2)` leaves a `0600` file holding a complete copy
    /// of every credential**, under a name nothing else looks at. Without this, `rm <store>` — the
    /// revocation this store documents — would leave the tokens on disk beside the file the operator
    /// just deleted, which makes the documented revocation wrong rather than merely incomplete.
    ///
    /// Reaped at `open` rather than at write, because the only writer that can leave one behind is a
    /// process that is no longer running: [`write_through`](Self::write_through) removes its own on
    /// failure, and there is exactly one in flight at a time. That reasoning depends on the
    /// single-process rule this module states; a second concurrent host would have its in-flight
    /// temporary removed under it, and would report the write as failed rather than lose data.
    ///
    /// Best effort throughout. A temporary that cannot be read or removed is not a reason to refuse
    /// to open a store that is otherwise sound — the credentials are the point, and the leftover is
    /// reported by [`stale_temporaries`](Self::stale_temporaries) for a caller that wants to look.
    fn reap_temporaries(&self) {
        for leftover in self.stale_temporaries() {
            let _ = std::fs::remove_file(leftover);
        }
    }

    /// Every unfinished temporary sitting beside this store, in no particular order.
    ///
    /// Public so the reaping above is observable rather than merely asserted — a test can plant one,
    /// see it listed, open the store and see the list empty.
    pub fn stale_temporaries(&self) -> Vec<PathBuf> {
        let (Some(directory), Some(name)) = (self.path.parent(), self.path.file_name()) else {
            return Vec::new();
        };
        let (prefix, suffix) = (format!(".{}.", name.to_string_lossy()), ".tmp");
        let Ok(entries) = std::fs::read_dir(directory) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(suffix))
            })
            .collect()
    }

    /// The file this store is kept in.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The layout this store renders through.
    pub fn layout(&self) -> &L {
        &self.layout
    }

    /// The path `reference` resolves to under this store's layout.
    ///
    /// The *address*, not the file — the same question [`MemoryStore::path`](crate::MemoryStore)
    /// answers.
    pub fn address(&self, reference: &CredentialRef) -> String {
        self.layout.render(reference)
    }

    /// The address a rendered path resolves back to — the inverse of [`address`](Self::address).
    ///
    /// # Errors
    ///
    /// [`StoreError::Layout`], carrying the layout's own explanation, when the path is not one this
    /// layout writes.
    pub fn reference(&self, path: &str) -> Result<CredentialRef, StoreError> {
        self.layout
            .parse(path)
            .map_err(|reason| StoreError::Layout { reason })
    }

    /// Every address currently holding a value, in order. Values are deliberately not exposed.
    pub fn paths(&self) -> Vec<String> {
        self.locked().keys().cloned().collect()
    }

    /// How many values are held.
    pub fn len(&self) -> usize {
        self.locked().len()
    }

    /// Whether the store holds nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[cfg(test)]
    fn inject_write_failure(&self) {
        self.fail_next_write.store(true, Ordering::SeqCst);
    }

    /// The map, with a poisoned lock recovered rather than propagated — as [`MemoryStore`].
    ///
    /// [`MemoryStore`]: crate::MemoryStore
    fn locked(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Secret>> {
        self.entries.lock().unwrap_or_else(|poisoned| {
            self.entries.clear_poison();
            poisoned.into_inner()
        })
    }

    fn locked_transactions(&self) -> std::sync::MutexGuard<'_, prepared::FileTransactions> {
        self.transactions.lock().unwrap_or_else(|poisoned| {
            self.transactions.clear_poison();
            poisoned.into_inner()
        })
    }

    /// Rewrite the whole file, atomically.
    ///
    /// **Atomic by the platform replacement primitive**, which is the only way to get it: a file
    /// opened for truncation is observably empty between the truncate and the write, and a crash in
    /// that window loses every credential rather than the one being written. The new contents go to
    /// a fresh owner-only sibling, are flushed, and then replace the old file in one step. A reader
    /// sees the whole previous version or the whole new one.
    ///
    /// The temporary lives in the **same directory** deliberately — replacement is atomic only
    /// within one filesystem/volume, and a temporary in a global scratch directory could be on
    /// another one. It is also created with `create_new`, so this never writes through a name
    /// somebody else placed there.
    fn write_through(&self, entries: &BTreeMap<String, Secret>) -> Result<(), StoreError> {
        let encoded_size =
            validate_candidate_bounds(entries).map_err(|reason| StoreError::Backend {
                path: self.path.display().to_string(),
                reason,
            })?;
        let mut rendered = String::with_capacity(encoded_size);
        rendered.push_str(HEADER);
        rendered.push('\n');
        for (address, secret) in entries {
            rendered.push_str(address);
            rendered.push(' ');
            rendered.push_str(&hex_encode(secret.expose_secret().as_bytes()));
            rendered.push('\n');
        }
        self.write_rendered_to(&self.path, &rendered, false)
    }

    fn write_live(
        &self,
        entries: &BTreeMap<String, Secret>,
        transactions: &prepared::FileTransactions,
    ) -> Result<(), StoreError> {
        if transactions.version_two {
            let rendered = prepared::encode_v2(entries, transactions).map_err(|reason| {
                StoreError::Backend {
                    path: self.path.display().to_string(),
                    reason,
                }
            })?;
            self.write_rendered_to(&self.path, &rendered, false)
        } else {
            self.write_through(entries)
        }
    }

    fn write_rendered_to(
        &self,
        destination: &Path,
        rendered: &str,
        require_directory_sync: bool,
    ) -> Result<(), StoreError> {
        let directory = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let temporary = directory.join(format!(
            ".{}.{}.{}.tmp",
            destination
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("credentials"),
            std::process::id(),
            NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));

        platform::ensure_directory(directory)?;
        // Refuse a destination widened or replaced after this store was opened before candidate
        // bytes are even rendered, much less written to a temporary. `replace` repeats the check
        // immediately before the atomic operation to cover the intervening race.
        platform::validate_destination(destination)?;
        let result =
            self.write_temporary(&temporary, destination, rendered, require_directory_sync);
        if result.is_err() {
            // Best effort: the write already failed, and a failure to clean up after it is not the
            // error worth reporting.
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    /// The fallible half of [`write_through`](Self::write_through), split out so the caller has one
    /// place to clean up from.
    fn write_temporary(
        &self,
        temporary: &Path,
        destination: &Path,
        rendered: &str,
        require_directory_sync: bool,
    ) -> Result<(), StoreError> {
        let mut file = platform::create_new(temporary, &self.path)?;
        crash_failpoint(destination, &self.path, "create");
        file.write_all(rendered.as_bytes())
            .map_err(|error| unreachable(&self.path, &error))?;
        crash_failpoint(destination, &self.path, "write");
        // Without this the rename can land before the bytes do, and a power cut leaves a file that
        // is present, correctly named, and empty.
        platform::flush(&file, &self.path)?;
        crash_failpoint(destination, &self.path, "flush");
        drop(file);

        #[cfg(test)]
        if self.fail_next_write.swap(false, Ordering::SeqCst) {
            return Err(StoreError::Unreachable {
                path: self.path.display().to_string(),
                reason: "injected failure before atomic replacement".to_owned(),
            });
        }

        platform::replace(temporary, destination)?;
        crash_failpoint(destination, &self.path, "replace");

        // The rename itself is a directory operation, so durability of *it* needs the directory
        // flushed. Prepared transactions require that proof before acknowledging a transition. The
        // pre-existing point-write contract keeps its best-effort behaviour because an error after
        // replacement would make its in-memory rollback disagree with the durable file.
        if let Some(directory) = temporary.parent() {
            let synced = platform::sync_directory(directory);
            if require_directory_sync {
                synced?;
            }
        }
        crash_failpoint(destination, &self.path, "directory-sync");

        Ok(())
    }
}

pub(crate) fn validate_candidate_bounds(
    entries: &BTreeMap<String, Secret>,
) -> Result<usize, String> {
    if entries.len() > MAX_ENTRIES {
        return Err(format!(
            "the store would contain {} entries, exceeding the {MAX_ENTRIES}-entry limit",
            entries.len()
        ));
    }
    if entries
        .values()
        .any(|secret| secret.expose_secret().len() > MAX_VALUE_BYTES)
    {
        return Err(format!(
            "a credential value exceeds the {MAX_VALUE_BYTES}-byte value limit; the value and its \
             address are omitted"
        ));
    }
    let encoded_size = entries
        .iter()
        .try_fold(HEADER.len() + 1, |size, (address, secret)| {
            let value_len = secret.expose_secret().len();
            size.checked_add(address.len())?
                .checked_add(1)?
                .checked_add(value_len.checked_mul(2)?)?
                .checked_add(1)
        });
    encoded_size
        .filter(|size| *size <= MAX_FILE_BYTES)
        .ok_or_else(|| {
            format!(
                "the encoded store would exceed the {MAX_FILE_BYTES}-byte limit; refusing the whole \
                 mutation rather than allocating or writing a partial file"
            )
        })
}

pub(crate) fn validate_transactional_bounds(
    entries: &BTreeMap<String, Secret>,
    terminal_records_after_decision: usize,
) -> Result<(), String> {
    let credential_bytes = validate_candidate_bounds(entries)?;
    // The committed form is the longest record grammar. Reserve one full line per terminal record,
    // plus the prepared record that successful prepare must be able to turn into a terminal without
    // discovering capacity after the coordinator's decision.
    const MAX_TRANSACTION_LINE_BYTES: usize = 160;
    let ledger_bytes = terminal_records_after_decision
        .checked_mul(MAX_TRANSACTION_LINE_BYTES)
        .and_then(|size| size.checked_add(prepared::HEADER_V2.len() + 1 + 36))
        .ok_or_else(|| "the transaction ledger size overflowed".to_owned())?;
    credential_bytes
        .checked_add(ledger_bytes)
        .filter(|size| *size <= MAX_FILE_BYTES)
        .map(|_| ())
        .ok_or_else(|| format!("the encoded store would exceed the {MAX_FILE_BYTES}-byte limit"))
}

#[async_trait]
impl<L: Layout + Send + Sync> SecretStore for FileStore<L> {
    async fn get(&self, reference: &CredentialRef) -> Result<Secret, StoreError> {
        let path = self.layout.render(reference);
        self.locked()
            .get(&path)
            .cloned()
            .ok_or(StoreError::NotFound { path })
    }

    async fn put(&self, reference: &CredentialRef, secret: &Secret) -> Result<(), StoreError> {
        let address = self.layout.render(reference);
        let transactions = self.locked_transactions();
        if transactions.prepared().is_some() {
            return Err(prepared_conflict(&self.path));
        }
        let mut entries = self.locked();
        let replaced = entries.insert(address.clone(), secret.clone());
        // Write while still holding the lock, so two concurrent writers cannot interleave a map
        // mutation with the other's file rewrite and persist a state neither of them held.
        if let Err(error) = self.write_live(&entries, &transactions) {
            // The file is the store. A value that reached the map but not the disk would be
            // resolvable until the next restart and gone after it, which is a worse failure than
            // refusing — so the map is put back the way it was and the caller is told.
            match replaced {
                Some(previous) => entries.insert(address, previous),
                None => entries.remove(&address),
            };
            return Err(error);
        }
        Ok(())
    }

    async fn delete(&self, reference: &CredentialRef) -> Result<(), StoreError> {
        let address = self.layout.render(reference);
        let transactions = self.locked_transactions();
        if transactions.prepared().is_some() {
            return Err(prepared_conflict(&self.path));
        }
        let mut entries = self.locked();
        // Idempotent, per the trait.
        let Some(previous) = entries.remove(&address) else {
            return Ok(());
        };
        if let Err(error) = self.write_live(&entries, &transactions) {
            entries.insert(address, previous);
            return Err(error);
        }
        Ok(())
    }

    async fn references(&self, scope: &CredentialScope) -> Result<Vec<CredentialRef>, StoreError> {
        self.locked()
            .keys()
            .map(|path| {
                self.layout
                    .parse(path)
                    .map_err(|reason| StoreError::Layout { reason })
            })
            .filter_map(|result| match result {
                Ok(reference) if scope.contains(&reference) => Some(Ok(reference)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    async fn apply(&self, mutations: &SecretBatch) -> Result<(), StoreError> {
        let transactions = self.locked_transactions();
        if transactions.prepared().is_some() {
            return Err(prepared_conflict(&self.path));
        }
        let mut entries = self.locked();
        let mut candidate = entries.clone();
        batch::apply_to(&mut candidate, &self.layout, mutations)?;
        // Persist the complete candidate before changing the in-process view. A write failure leaves
        // both representations at the old state, matching the point methods' rollback guarantee.
        self.write_live(&candidate, &transactions)?;
        *entries = candidate;
        Ok(())
    }
}

/// Path and entry count. **Never a value, and never an address** — a derived `Debug` would print
/// every key, and C-159 is this repository's precedent for a derived rendering becoming the leak.
impl<L> fmt::Debug for FileStore<L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FileStore(<opaque>)")
    }
}

/// An IO failure, as a [`StoreError`].
///
/// `std::io::Error`'s own message names an errno and, for the calls used here, nothing else. It
/// never carries file contents, so this cannot carry a value — which is why the reason is passed
/// through rather than flattened to "an IO error".
pub(super) fn unreachable(path: &Path, error: &std::io::Error) -> StoreError {
    StoreError::Unreachable {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}

fn prepared_conflict(path: &Path) -> StoreError {
    StoreError::Conflict {
        path: path.display().to_string(),
        reason: "a prepared secret transaction owns the mutation slot".to_owned(),
    }
}

fn fixed_sibling(store: &Path, suffix: &str) -> PathBuf {
    let directory = store
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = store
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("credentials");
    directory.join(format!(".{name}.{suffix}"))
}

fn remove_fixed(path: &Path, store: &Path) -> Result<(), StoreError> {
    let Some(file) = platform::open_existing(path)? else {
        return Ok(());
    };
    drop(file);
    std::fs::remove_file(path).map_err(|error| unreachable(store, &error))?;
    if let Some(directory) = path.parent() {
        platform::sync_directory(directory)?;
    }
    crash_failpoint(path, store, "cleanup");
    Ok(())
}

#[cfg(test)]
fn crash_failpoint(destination: &Path, store: &Path, boundary: &str) {
    let target = if destination == fixed_sibling(store, "prepared") {
        "stage"
    } else {
        "live"
    };
    let expected = format!("{target}:{boundary}");
    if std::env::var("CONNECTOR_SECRETS_CRASH_AT").ok().as_deref() == Some(expected.as_str()) {
        std::process::abort();
    }
}

#[cfg(not(test))]
fn crash_failpoint(_destination: &Path, _store: &Path, _boundary: &str) {}

fn recover_stage<L: Layout>(
    store: &Path,
    layout: &L,
    live: &mut prepared::FileTransactions,
) -> Result<(), StoreError> {
    let stage_path = fixed_sibling(store, "prepared");
    let Some(mut stage_file) = platform::open_existing(&stage_path)? else {
        if live.prepared().is_some() {
            return Err(StoreError::Backend {
                path: store.display().to_string(),
                reason: "the live store is prepared but its complete fixed stage is absent"
                    .to_owned(),
            });
        }
        return Ok(());
    };
    let stage_contents = read_bounded(&mut stage_file, &stage_path)?;
    let (stage_entries, stage) = prepared::parse_v2(&stage_contents, layout, &stage_path)?;
    if stage.prepared().is_some() || stage.retired_through != live.retired_through {
        return Err(StoreError::Backend {
            path: store.display().to_string(),
            reason: "the fixed stage does not match the live transaction ledger".to_owned(),
        });
    }

    if let Some((id, digest)) = live.prepared() {
        let mut expected = live.records.clone();
        expected.insert(id.key(), prepared::FileRecord::Committed(digest));
        if stage.records != expected {
            return Err(StoreError::Backend {
                path: store.display().to_string(),
                reason: "the fixed stage does not match the live prepared record".to_owned(),
            });
        }
        live.candidate = Some(prepared::Candidate {
            id,
            digest,
            entries: stage_entries,
        });
        return Ok(());
    }

    let mut differing = stage
        .records
        .keys()
        .chain(live.records.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    differing.retain(|id| stage.records.get(id) != live.records.get(id));
    let differing = differing.into_iter().collect::<Vec<_>>();
    let cleanup_only = match differing.as_slice() {
        [] => true,
        [id] => matches!(
            (stage.records.get(id), live.records.get(id)),
            (
                Some(prepared::FileRecord::Committed(_)),
                None | Some(prepared::FileRecord::Aborted)
            )
        ),
        _ => false,
    };
    if !cleanup_only {
        return Err(StoreError::Backend {
            path: store.display().to_string(),
            reason:
                "the leftover fixed stage has no unambiguous recovery relation to the live store"
                    .to_owned(),
        });
    }
    remove_fixed(&stage_path, store)
}

fn read_bounded(file: &mut File, path: &Path) -> Result<String, StoreError> {
    let length = file
        .metadata()
        .map_err(|error| unreachable(path, &error))?
        .len();
    if length > MAX_FILE_BYTES as u64 {
        return Err(StoreError::Backend {
            path: path.display().to_string(),
            reason: format!(
                "the store is {length} bytes, exceeding the {MAX_FILE_BYTES}-byte limit; contents \
                 were not read"
            ),
        });
    }

    let mut bytes = Vec::with_capacity(length as usize);
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| unreachable(path, &error))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(StoreError::Backend {
            path: path.display().to_string(),
            reason: format!(
                "the store grew beyond the {MAX_FILE_BYTES}-byte limit while it was being read; \
                 contents were refused"
            ),
        });
    }
    String::from_utf8(bytes).map_err(|_| StoreError::Backend {
        path: path.display().to_string(),
        reason: "the store is not UTF-8".to_owned(),
    })
}

/// Read a whole store file into a map, refusing anything it cannot account for.
///
/// **Nothing is skipped.** A line this cannot read is a credential this store would silently stop
/// resolving, and "the connector is not connected" is the wrong answer to "the file is damaged" for
/// exactly the reason [`StoreError::NotFound`] and [`StoreError::Unreachable`] are different types.
///
/// No message below quotes a value or any part of one — a hex field that failed to decode is
/// reported by its line number and its length, never its content.
fn parse<L: Layout>(
    contents: &str,
    layout: &L,
    file: &Path,
) -> Result<BTreeMap<String, Secret>, StoreError> {
    let mut entries = BTreeMap::new();
    for (index, line) in contents.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();

        // **The version is read, not skipped.** It sits behind a `#` so an operator sees a comment,
        // but a comment nothing checks would let a future format load as this one.
        if let Some(version) = line.strip_prefix(VERSION_PREFIX) {
            if version != VERSION {
                return Err(StoreError::Backend {
                    path: file.display().to_string(),
                    reason: format!(
                        "line {number}: this file says it is version {version:?} and this build \
                         speaks version {VERSION:?}. Refusing rather than reading it as {VERSION:?} \
                         — a credential store that guesses at a format it does not know hands back \
                         a wrong value instead of an error."
                    ),
                });
            }
            continue;
        }

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let bad = |reason: String| StoreError::Backend {
            path: file.display().to_string(),
            reason: format!("line {number}: {reason}"),
        };

        let (address, encoded) = line
            .split_once(' ')
            .ok_or_else(|| bad("expected `<address> <hex>`, and there is no space".to_owned()))?;

        if encoded.len() > MAX_VALUE_BYTES * 2 {
            return Err(bad(format!(
                "the encoded value is {} characters, exceeding the {}-byte decoded-value limit",
                encoded.len(),
                MAX_VALUE_BYTES
            )));
        }

        // The address must be one this layout writes, and must be spelled the way this layout
        // spells it. Both halves matter: the first is what keeps a hand-edited file from inventing
        // a tenant, and the second is what keeps one credential from having two entries.
        let reference = layout.parse(address).map_err(&bad)?;
        let canonical = layout.render(&reference);
        if canonical != address {
            return Err(bad(format!(
                "{address:?} is not how this layout spells the address it names ({canonical:?}), \
                 so one credential would have two entries"
            )));
        }

        let value = hex_decode(encoded).ok_or_else(|| {
            bad(format!(
                "the value field is {} characters and is not valid hex",
                encoded.len()
            ))
        })?;
        let value = String::from_utf8(value)
            .map_err(|_| bad("the value does not decode to UTF-8".to_owned()))?;

        if entries.insert(canonical, Secret::new(value)).is_some() {
            return Err(bad(format!(
                "{address:?} appears more than once, and nothing here can say which is current"
            )));
        }
        if entries.len() > MAX_ENTRIES {
            return Err(bad(format!(
                "the store contains more than the {MAX_ENTRIES}-entry limit"
            )));
        }
    }
    Ok(entries)
}

/// Lowercase hex. **Framing, not protection** — see the module documentation.
fn hex_encode(bytes: &[u8]) -> String {
    use fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Infallible: writing to a `String` cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The inverse of [`hex_encode`], or `None` for anything that is not lowercase-or-uppercase hex of
/// even length.
fn hex_decode(encoded: &str) -> Option<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) {
        return None;
    }
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out.push((high * 16 + low) as u8);
    }
    Some(out)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    /// Obviously not a credential, and long enough that a redactor would hold it.
    const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-file-store";

    /// A directory of this test's own, removed when the guard drops.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "connector-secrets-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }

        /// The store file inside it — one directory level down, so directory creation is exercised.
        fn store(&self) -> PathBuf {
            self.0.join("store").join("credentials")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn reference() -> CredentialRef {
        CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token").expect("valid")
    }

    #[tokio::test]
    async fn an_atomic_move_survives_reopen_as_one_state() {
        let scratch = Scratch::new("batch-move");
        let path = scratch.store();
        let source = reference();
        let destination = CredentialRef::for_instance(
            "9f3a4b2c",
            "com.zendesk.api",
            "0d3f79ae-b6df-4f77-8f77-438436c3b2ef",
            "support",
            "api_token",
        )
        .expect("valid instance address");
        let store = FileStore::open(&path).expect("open");
        store
            .put(&source, &Secret::new(SENTINEL))
            .await
            .expect("put");
        let scope = CredentialScope::new("9f3a4b2c", "com.zendesk.api").expect("scope");
        let mut batch = SecretBatch::new(scope.clone());
        batch
            .move_secret(source.clone(), destination.clone())
            .expect("in scope");
        store.apply(&batch).await.expect("atomic move");
        drop(store);

        let reopened = FileStore::open(&path).expect("reopen");
        assert!(reopened.get(&source).await.unwrap_err().is_not_found());
        assert_eq!(
            reopened
                .get(&destination)
                .await
                .expect("destination")
                .expose_secret(),
            SENTINEL
        );
        assert_eq!(
            reopened.references(&scope).await.expect("inventory"),
            vec![destination]
        );
    }

    fn mode_of(path: &Path) -> u32 {
        std::fs::metadata(path)
            .expect("it exists")
            .permissions()
            .mode()
            & 0o777
    }

    /// **The point of the type.** A value stored through one instance is readable through the next
    /// one built over the same file.
    #[tokio::test]
    async fn a_value_survives_the_store_being_dropped_and_reopened() {
        let scratch = Scratch::new("round-trip");
        let path = scratch.store();

        {
            let store = FileStore::open(&path).expect("open");
            store
                .put(&reference(), &Secret::new(SENTINEL))
                .await
                .expect("put");
        }

        let reopened = FileStore::open(&path).expect("reopen");
        assert_eq!(
            reopened
                .get(&reference())
                .await
                .expect("get")
                .expose_secret(),
            SENTINEL
        );

        reopened.delete(&reference()).await.expect("delete");
        drop(reopened);
        let again = FileStore::open(&path).expect("reopen");
        assert!(again.get(&reference()).await.unwrap_err().is_not_found());
    }

    /// **The full address round-trips, not merely the value.**
    ///
    /// Two tenants of one vendor and two services of one vendor are the two ways a store keyed
    /// loosely collides — the first would undo C-204's per-account tenancy and the second C-219's
    /// per-service addressing, and both would do it silently, by handing back *a* credential.
    #[tokio::test]
    async fn nothing_collides_across_tenants_or_across_services_of_one_vendor() {
        let scratch = Scratch::new("addressing");
        let path = scratch.store();

        let addresses = [
            ("tenant-a", "com.zendesk.api", "support", "api_token"),
            ("tenant-b", "com.zendesk.api", "support", "api_token"),
            ("tenant-a", "com.contentful.api", "delivery", "api_token"),
            ("tenant-a", "com.contentful.api", "management", "api_token"),
            ("tenant-a", "com.anthropic.api", "default", "api_key"),
        ];

        {
            let store = FileStore::open(&path).expect("open");
            for (tenant, authority, service, credential) in addresses {
                let reference = CredentialRef::new(tenant, authority, service, credential)
                    .expect("a valid address");
                store
                    .put(
                        &reference,
                        &Secret::new(format!("{SENTINEL}/{tenant}/{authority}/{service}")),
                    )
                    .await
                    .expect("put");
            }
            assert_eq!(store.len(), addresses.len(), "two addresses collided");
        }

        let reopened = FileStore::open(&path).expect("reopen");
        assert_eq!(reopened.len(), addresses.len(), "the reload lost an entry");
        for (tenant, authority, service, credential) in addresses {
            let reference =
                CredentialRef::new(tenant, authority, service, credential).expect("valid");
            assert_eq!(
                reopened.get(&reference).await.expect("get").expose_secret(),
                format!("{SENTINEL}/{tenant}/{authority}/{service}"),
                "{tenant}/{authority}/{service} came back with another address's value"
            );
        }
    }

    /// `0600` and `0700`, on a store that has only just been created.
    #[test]
    fn a_fresh_store_is_0600_inside_a_0700_directory() {
        let scratch = Scratch::new("modes");
        let path = scratch.store();
        let store = FileStore::open(&path).expect("open");

        assert!(path.exists(), "`open` did not create the file");
        assert_eq!(
            mode_of(store.path()),
            FILE_MODE,
            "the store file is not 0600"
        );
        assert_eq!(
            mode_of(path.parent().expect("a parent")),
            DIR_MODE,
            "the containing directory is not 0700"
        );
    }

    /// A file somebody else can read is refused rather than repaired.
    #[test]
    fn a_world_readable_store_is_refused() {
        let scratch = Scratch::new("mode-refusal");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open"));
        std::fs::write(
            &path,
            format!(
                "{HEADER}\ntenants/9f3a4b2c/com.zendesk.api/support/api_token {}\n",
                hex_encode(SENTINEL.as_bytes())
            ),
        )
        .expect("plant a real entry");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("loosen it");
        let before = std::fs::read(&path).expect("prior bytes");

        let error = FileStore::open(&path).expect_err("a 0644 store must be refused");
        assert!(
            matches!(error, StoreError::Denied { .. }),
            "expected a refusal, got {error:?}"
        );
        assert!(
            error.to_string().contains("0644"),
            "the refusal must say what the mode is: {error}"
        );
        assert!(!error.to_string().contains(SENTINEL));
        assert!(!error.to_string().contains("com.zendesk.api"));
        assert_eq!(std::fs::read(&path).expect("bytes remain"), before);
        assert_eq!(
            mode_of(&path),
            0o644,
            "the store tightened the mode instead of reporting it"
        );
    }

    /// **A directory somebody else can enter is refused too**, on the same reasoning as the file.
    ///
    /// Separate from the file case because they are separate calls: deleting the directory leg of
    /// [`ensure_directory`] left every file-mode assertion green, so `0700` was a behaviour nothing
    /// observed.
    #[test]
    fn a_world_readable_directory_is_refused() {
        let scratch = Scratch::new("dir-mode-refusal");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open"));
        std::fs::write(
            &path,
            format!(
                "{HEADER}\ntenants/9f3a4b2c/com.zendesk.api/support/api_token {}\n",
                hex_encode(SENTINEL.as_bytes())
            ),
        )
        .expect("plant a real entry");
        let directory = path.parent().expect("a parent").to_owned();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
            .expect("loosen it");

        let error = FileStore::open(&path).expect_err("a 0755 directory must be refused");

        let unsafe_mode = mode_of(&directory);
        let bytes = std::fs::read(&path).expect("prior bytes");

        assert_eq!(unsafe_mode, 0o755, "refusal repaired the directory");
        assert_eq!(std::fs::read(&path).expect("bytes remain"), bytes);
        assert!(!error.to_string().contains(SENTINEL));
        assert!(!error.to_string().contains("com.zendesk.api"));

        // Restored after evidence so a failure still leaves a removable directory.
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(DIR_MODE))
            .expect("restore");

        assert!(
            matches!(error, StoreError::Denied { .. }),
            "expected a refusal, got {error:?}"
        );
        assert!(
            error.to_string().contains("0755"),
            "the refusal must say what the mode is: {error}"
        );
    }

    /// A store directly beneath a shared parent is still unsafe, but the operator must never be
    /// told to narrow that shared parent for every other user of the machine.
    #[test]
    fn a_shared_parent_refusal_recommends_an_owner_only_child_not_chmodding_the_parent() {
        use std::os::unix::fs::MetadataExt as _;

        // `std::env::temp_dir()` is a genuinely shared 01777 directory on Linux, but on macOS it
        // normally resolves to a private per-user directory. Construct the unsafe direct parent so
        // this test proves the security predicate instead of assuming a platform's temp layout.
        let scratch = Scratch::new("shared-parent-refusal");
        let parent = scratch.0.join("shared");
        std::fs::create_dir_all(&parent).expect("create deliberately shared parent");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777))
            .expect("make the direct parent non-owner-only");
        let path = parent.join("credentials.store");
        let before = std::fs::symlink_metadata(&parent).expect("inspect shared parent");
        let snapshot = (
            before.mode(),
            before.uid(),
            before.gid(),
            before.dev(),
            before.ino(),
        );

        let error = FileStore::open(&path).expect_err("a shared direct parent must be refused");
        let message = error.to_string();
        assert!(message.contains(&parent.display().to_string()), "{message}");
        assert!(message.contains("owner-only child"), "{message}");
        assert!(message.contains("per-user state"), "{message}");
        assert!(!message.contains("chmod 700"), "{message}");
        assert!(!message.contains("com.zendesk.api"), "{message}");
        assert!(!message.contains(SENTINEL), "{message}");
        assert!(!path.exists(), "a refusal must not create the store");

        let after = std::fs::symlink_metadata(&parent).expect("reinspect shared parent");
        assert_eq!(
            snapshot,
            (
                after.mode(),
                after.uid(),
                after.gid(),
                after.dev(),
                after.ino(),
            ),
            "refusal changed the shared parent's security metadata"
        );
    }

    #[test]
    fn an_oversized_store_is_refused_before_its_contents_are_parsed() {
        const EXPECTED_LIMIT: usize = 1024 * 1024;

        let scratch = Scratch::new("bounded-read");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open"));
        std::fs::write(&path, format!("#{}", "x".repeat(EXPECTED_LIMIT)))
            .expect("plant an oversized store");

        let error = FileStore::open(&path).expect_err("an oversized store must be refused");
        let message = error.to_string();
        assert!(
            message.contains("1048576"),
            "the bound must be named: {message}"
        );
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(
            !message.contains(&"x".repeat(64)),
            "contents leaked: {message}"
        );
    }

    /// **A temporary left by a crash is reaped**, because `rm <store>` must actually revoke.
    ///
    /// A write that dies between the `write_all` and the `rename(2)` leaves a `0600` file holding a
    /// complete copy of every credential under a name nothing looks at. An operator who then deletes
    /// the store believes they have revoked; the tokens are still on disk beside it.
    #[test]
    fn a_temporary_left_by_a_crash_is_reaped_on_the_next_open() {
        let scratch = Scratch::new("reap");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open"));

        // What a crash between `write_all` and `rename` leaves: the temporary's real name shape,
        // holding a real entry.
        let orphan = path.parent().expect("a parent").join(format!(
            ".{}.999999.0.tmp",
            path.file_name().expect("a name").to_string_lossy()
        ));
        std::fs::write(
            &orphan,
            format!(
                "{HEADER}\ntenants/a/com.acme.api/token {}\n",
                hex_encode(SENTINEL.as_bytes())
            ),
        )
        .expect("plant the orphan");
        assert!(orphan.exists());

        let store = FileStore::open(&path).expect("reopen");

        assert!(
            !orphan.exists(),
            "a crashed write's temporary survived the next open, so `rm {}` would leave a full \
             copy of every credential beside the file the operator deleted",
            path.display()
        );
        assert!(
            store.stale_temporaries().is_empty(),
            "the store still reports leftovers after reaping them"
        );
    }

    /// A file from a format this build does not speak is refused, not read as this one.
    #[test]
    fn a_file_from_another_version_is_refused() {
        let scratch = Scratch::new("version");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open"));
        std::fs::write(
            &path,
            format!(
                "{VERSION_PREFIX}3\ntenants/a/com.acme.api/token {}\n",
                hex_encode(SENTINEL.as_bytes())
            ),
        )
        .expect("write a v3 file");

        let error = FileStore::open(&path).expect_err("a v3 file must not load as v1 or v2");
        let message = error.to_string();
        assert!(matches!(error, StoreError::Backend { .. }), "{error:?}");
        assert!(
            message.contains("\"3\""),
            "the refusal must name it: {message}"
        );
        assert!(!message.contains(SENTINEL));

        // And the version this build does write is the one it accepts, so the check above cannot be
        // satisfied by refusing everything.
        std::fs::write(&path, format!("{HEADER}\n")).expect("write a v1 file");
        assert!(FileStore::open(&path).is_ok(), "a v1 file must still load");
    }

    /// **The three legs of the atomic write, audited over this file's own source.**
    ///
    /// Each of `create_new`, the temporary's `sync_all` and the directory `fsync` can be deleted
    /// with the whole suite staying green — they are durability and anti-clobber properties, and a
    /// single-threaded test on a working filesystem cannot observe any of them. That is exactly the
    /// case this repository already handles by reading its own source
    /// (`Secret::every_exit_from_the_wrapper_is_named_expose_secret`), so the same instrument is
    /// used here rather than leaving three deletions undefended.
    ///
    /// It asserts over the write path only, and it fails loudly if it stops matching anything, so it
    /// cannot quietly become an assertion about nothing.
    #[test]
    fn the_write_path_keeps_all_three_legs_of_its_atomicity() {
        let unix = include_str!("file/unix.rs");
        let windows = include_str!("file/windows.rs");
        for (source, fragment, why) in [
            (
                unix,
                "OFlags::CREATE | OFlags::EXCL",
                "Unix must create a fresh temporary",
            ),
            (
                unix,
                "OFlags::NOFOLLOW",
                "Unix must not follow a planted temporary",
            ),
            (
                unix,
                "file.sync_all()",
                "Unix must flush bytes before rename",
            ),
            (unix, "std::fs::rename", "Unix replacement must be atomic"),
            (
                windows,
                "CREATE_NEW",
                "Windows must create a fresh temporary",
            ),
            (
                windows,
                "FlushFileBuffers",
                "Windows must flush bytes before replacement",
            ),
            (
                windows,
                "ReplaceFileW",
                "Windows existing-target replacement must be atomic",
            ),
            (
                windows,
                "MoveFileExW",
                "Windows first install must be same-directory atomic",
            ),
        ] {
            assert!(
                source.contains(fragment),
                "the atomic write no longer contains `{fragment}` — {why}. If this was deliberate, \
                 the module documentation and this test both have to say so."
            );
        }
    }

    /// **A reader never observes a partial store while a writer is rewriting it.**
    ///
    /// The behavioural half of the audit above, and the assertion
    /// `a_write_leaves_no_temporary_and_no_truncated_file` only appeared to make: that test opened
    /// the store *after* the writes finished, so a truncate-then-write implementation left it green
    /// and the truncation window in its name went unobserved. This one reads while the writes are
    /// happening.
    ///
    /// It can only fail in one direction. Under `rename(2)` every observation is a whole file, so
    /// the assertion holds for every interleaving; under truncate-then-write a reader eventually
    /// lands in the window and sees a store that is empty, short, or unparseable.
    #[test]
    fn a_concurrent_reader_never_sees_a_half_written_store() {
        let scratch = Scratch::new("torn-read");
        let path = scratch.store();
        let store = FileStore::open(&path).expect("open");

        // Ten entries, so a torn read is a visibly short file rather than an ambiguous one.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime");
        for index in 0..10 {
            let reference = CredentialRef::new(
                format!("tenant-{index}").as_str(),
                "com.acme.api",
                "default",
                "token",
            )
            .expect("valid");
            runtime
                .block_on(store.put(&reference, &Secret::new(format!("{SENTINEL}-{index}"))))
                .expect("put");
        }

        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reader = {
            let (path, stop) = (path.clone(), std::sync::Arc::clone(&stop));
            std::thread::spawn(move || {
                let mut observations = 0usize;
                while !stop.load(Ordering::Relaxed) {
                    // **The bytes, not `FileStore::open`.** Opening a store reaps stale
                    // temporaries, and a reader that did that would delete the writer's in-flight
                    // one — which is the single-process rule this module states, demonstrated the
                    // hard way when this test was first written that way. Reading the file directly
                    // is also the truer question: atomicity is a property of what is on disk, not of
                    // what a second `FileStore` makes of it.
                    let seen = match std::fs::read_to_string(&path) {
                        Ok(contents) => contents,
                        // `rename(2)` never unlinks the destination, so the path is never absent.
                        Err(error) => panic!("a reader could not read the store: {error}"),
                    };
                    let lines: Vec<&str> = seen
                        .lines()
                        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
                        .collect();

                    // The writer only ever *replaces* one of the ten values below, so a whole file
                    // always has a header and exactly ten entries. Anything else was caught in a
                    // truncation window.
                    assert!(
                        seen.starts_with(HEADER),
                        "a reader saw a file that does not begin with the header, so it caught the \
                         store mid-write"
                    );
                    assert_eq!(
                        lines.len(),
                        10,
                        "a reader saw {} entries rather than 10, so it caught the store mid-write",
                        lines.len()
                    );
                    observations += 1;
                }
                observations
            })
        };

        let target =
            CredentialRef::new("tenant-0", "com.acme.api", "default", "token").expect("valid");
        for index in 0..300 {
            runtime
                .block_on(store.put(&target, &Secret::new(format!("{SENTINEL}-rewrite-{index}"))))
                .expect("put");
        }
        stop.store(true, Ordering::Relaxed);

        let observations = reader.join().expect("the reader thread did not panic");
        assert!(
            observations > 0,
            "the reader never managed to open the store, so it asserted nothing"
        );
    }

    /// A rewrite leaves no temporary behind, and the file is whole at every point a reader could
    /// look at it.
    #[tokio::test]
    async fn a_write_leaves_no_temporary_and_no_truncated_file() {
        let scratch = Scratch::new("atomic");
        let path = scratch.store();
        let store = FileStore::open(&path).expect("open");

        for index in 0..8 {
            let reference = CredentialRef::new(
                format!("tenant-{index}").as_str(),
                "com.acme.api",
                "default",
                "token",
            )
            .expect("valid");
            store
                .put(&reference, &Secret::new(format!("{SENTINEL}-{index}")))
                .await
                .expect("put");
        }

        let leftovers: Vec<String> = std::fs::read_dir(path.parent().expect("a parent"))
            .expect("read the directory")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "credentials" && name != ".credentials.lease")
            .collect();
        assert!(
            leftovers.is_empty(),
            "the directory holds files the store did not mean to leave: {leftovers:?}"
        );
        drop(store);
        assert_eq!(FileStore::open(&path).expect("reopen").len(), 8);
    }

    /// A file this store cannot account for is an error, never a quietly emptier store.
    ///
    /// Silently skipping a line it could not read would report the tenant as *not connected*, which
    /// is precisely the confusion [`StoreError::NotFound`] and [`StoreError::Unreachable`] are two
    /// types in order to avoid — and it would invite the operator to paste the token again.
    #[test]
    fn a_damaged_file_is_refused_rather_than_partly_loaded() {
        for (label, contents) in [
            ("no separator", "tenants/a/com.acme.api/token\n"),
            ("bad hex", "tenants/a/com.acme.api/token zzzz\n"),
            ("odd hex", "tenants/a/com.acme.api/token abc\n"),
            ("not an address", "wherever/a/token 4141\n"),
            (
                "the elided service spelled out",
                "tenants/a/com.acme.api/default/token 4141\n",
            ),
            (
                "the same address twice",
                "tenants/a/com.acme.api/token 4141\ntenants/a/com.acme.api/token 4242\n",
            ),
        ] {
            let scratch = Scratch::new("damaged");
            let path = scratch.store();
            drop(FileStore::open(&path).expect("open"));
            std::fs::write(&path, contents).expect("write the damaged file");

            let error = FileStore::open(&path)
                .err()
                .unwrap_or_else(|| panic!("a store with {label} was loaded rather than refused"));
            assert!(
                matches!(error, StoreError::Backend { .. }),
                "{label}: expected a `Backend` refusal, got {error:?}"
            );
        }
    }

    /// Nothing a damaged file produces quotes the bytes it could not read.
    #[test]
    fn a_parse_failure_names_the_line_and_never_the_value() {
        let scratch = Scratch::new("parse-message");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open"));
        std::fs::write(
            &path,
            format!(
                "{HEADER}\ntenants/a/com.acme.api/token {}!!\n",
                hex_encode(SENTINEL.as_bytes())
            ),
        )
        .expect("write");

        let error = FileStore::open(&path).expect_err("invalid hex must be refused");
        let message = error.to_string();
        assert!(matches!(error, StoreError::Backend { .. }), "{error:?}");
        assert!(message.contains("line 2"), "no line number: {message}");
        assert!(
            !message.contains(&hex_encode(SENTINEL.as_bytes())),
            "the message quoted the undecodable field: {message}"
        );
        assert!(!message.contains(SENTINEL), "the message quoted a value");
    }

    /// The `Debug` rendering, which is the one C-159 warns about.
    #[tokio::test]
    async fn debug_carries_neither_a_value_nor_an_address() {
        let scratch = Scratch::new("debug");
        let store = FileStore::open(scratch.store()).expect("open");
        store
            .put(&reference(), &Secret::new(SENTINEL))
            .await
            .expect("put");

        let rendered = format!("{store:?}");
        assert!(!rendered.contains(SENTINEL), "Debug served a value");
        assert!(
            !rendered.contains("com.zendesk.api"),
            "Debug served an address: {rendered}"
        );
    }

    /// Neither does the error a failed write produces. The failure is manufactured by making the
    /// directory unwritable, which is the closest thing to a full disk a test can arrange.
    #[tokio::test]
    async fn a_write_failure_names_no_value() {
        let scratch = Scratch::new("write-failure");
        let path = scratch.store();
        let store = FileStore::open(&path).expect("open");
        let directory = path.parent().expect("a parent").to_owned();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500))
            .expect("make it read-only");

        let error = store
            .put(&reference(), &Secret::new(SENTINEL))
            .await
            .expect_err("a read-only directory must refuse a write");

        // Restored before the assertions so a failure still cleans up.
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(DIR_MODE))
            .expect("restore");

        let message = error.to_string();
        assert!(
            !message.contains(SENTINEL),
            "the write error quoted the value"
        );
        assert!(
            message.contains("credentials"),
            "the write error should name the file: {message}"
        );
        // And the value did not half-land: the map was rolled back with the file.
        assert!(store.get(&reference()).await.unwrap_err().is_not_found());
    }

    #[test]
    fn a_store_symlink_is_refused_without_reading_or_changing_either_object() {
        use std::os::unix::fs::symlink;

        let scratch = Scratch::new("store-symlink");
        let real = scratch.store();
        drop(FileStore::open(&real).expect("open real store"));
        std::fs::write(
            &real,
            format!(
                "{HEADER}\ntenants/9f3a4b2c/com.zendesk.api/support/api_token {}\n",
                hex_encode(SENTINEL.as_bytes())
            ),
        )
        .expect("plant target");
        let link = real.parent().expect("parent").join("linked-credentials");
        symlink(&real, &link).expect("plant symlink");
        let before_link = std::fs::symlink_metadata(&link).expect("link metadata");
        let before_target = std::fs::read(&real).expect("target bytes");

        let error = FileStore::open(&link).expect_err("a symlink must be refused");
        let message = error.to_string();
        assert!(message.contains(&link.display().to_string()), "{message}");
        assert!(!message.contains(SENTINEL), "{message}");
        assert_eq!(std::fs::read(&real).expect("target remains"), before_target);
        let after_link = std::fs::symlink_metadata(&link).expect("link remains");
        assert_eq!(before_link.ino(), after_link.ino());
        assert_eq!(before_link.mode(), after_link.mode());
    }

    #[test]
    fn a_directory_symlink_is_refused_without_changing_it() {
        use std::os::unix::fs::symlink;

        let scratch = Scratch::new("directory-symlink");
        let real = scratch.0.join("real-state");
        drop(FileStore::open(real.join("credentials")).expect("create real directory"));
        let link = scratch.0.join("linked-state");
        symlink(&real, &link).expect("plant directory symlink");
        let before = std::fs::symlink_metadata(&link).expect("link metadata");

        let path = link.join("credentials");
        let error = FileStore::open(&path).expect_err("a directory symlink must be refused");
        let message = error.to_string();
        assert!(message.contains(&link.display().to_string()), "{message}");
        assert!(!message.contains(SENTINEL), "{message}");
        let after = std::fs::symlink_metadata(&link).expect("link remains");
        assert_eq!(before.ino(), after.ino());
        assert_eq!(before.mode(), after.mode());
    }

    #[test]
    fn wrong_object_kinds_are_refused_without_repair() {
        let scratch = Scratch::new("wrong-kinds");
        let store_as_directory = scratch.store();
        std::fs::create_dir_all(store_as_directory.parent().expect("parent")).expect("parents");
        std::fs::set_permissions(
            store_as_directory.parent().expect("parent"),
            std::fs::Permissions::from_mode(DIR_MODE),
        )
        .expect("secure parent");
        std::fs::create_dir(&store_as_directory).expect("directory at store path");
        let before = std::fs::symlink_metadata(&store_as_directory).expect("metadata");
        let error =
            FileStore::open(&store_as_directory).expect_err("directory is not a store file");
        assert!(matches!(error, StoreError::Denied { .. }), "{error:?}");
        let after = std::fs::symlink_metadata(&store_as_directory).expect("metadata remains");
        assert_eq!((before.ino(), before.mode()), (after.ino(), after.mode()));

        let file_as_directory = scratch.0.join("plain-file");
        std::fs::write(&file_as_directory, b"not a directory").expect("plant file");
        std::fs::set_permissions(
            &file_as_directory,
            std::fs::Permissions::from_mode(FILE_MODE),
        )
        .expect("secure file");
        let before = std::fs::symlink_metadata(&file_as_directory).expect("metadata");
        let path = file_as_directory.join("credentials");
        let error = FileStore::open(&path).expect_err("file is not a state directory");
        assert!(matches!(error, StoreError::Denied { .. }), "{error:?}");
        let after = std::fs::symlink_metadata(&file_as_directory).expect("metadata remains");
        assert_eq!((before.ino(), before.mode()), (after.ino(), after.mode()));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_fifo_swap_is_refused_without_blocking_or_reading() {
        let scratch = Scratch::new("fifo");
        let path = scratch.store();
        std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
        std::fs::set_permissions(
            path.parent().expect("parent"),
            std::fs::Permissions::from_mode(DIR_MODE),
        )
        .expect("secure parent");
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &path,
            rustix::fs::Mode::from_raw_mode(FILE_MODE),
        )
        .expect("plant fifo");
        let before = std::fs::symlink_metadata(&path).expect("fifo metadata");

        let error = FileStore::open(&path).expect_err("fifo must be refused without opening it");
        assert!(matches!(error, StoreError::Denied { .. }), "{error:?}");
        let after = std::fs::symlink_metadata(&path).expect("fifo remains");
        assert_eq!((before.ino(), before.mode()), (after.ino(), after.mode()));
    }

    #[test]
    fn an_unopenable_store_is_refused_without_reading_or_repair() {
        let scratch = Scratch::new("unopenable");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open"));
        std::fs::write(
            &path,
            format!(
                "{HEADER}\ntenants/9f3a4b2c/com.zendesk.api/support/api_token {}\n",
                hex_encode(SENTINEL.as_bytes())
            ),
        )
        .expect("plant a real entry");
        let bytes = std::fs::read(&path).expect("prior bytes");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
            .expect("make unopenable");
        let before = std::fs::symlink_metadata(&path).expect("metadata");

        let error = FileStore::open(&path).expect_err("unopenable metadata handle must refuse");
        let message = error.to_string();
        assert!(!message.contains(SENTINEL), "{message}");
        assert!(!message.contains("com.zendesk.api"), "{message}");
        let after = std::fs::symlink_metadata(&path).expect("metadata remains");
        assert_eq!((before.uid(), before.mode()), (after.uid(), after.mode()));

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(FILE_MODE))
            .expect("restore for cleanup");
        assert_eq!(std::fs::read(&path).expect("bytes remain"), bytes);
    }

    #[tokio::test]
    async fn mutations_revalidate_widened_directory_and_file_before_writing_a_temporary() {
        for widen_directory in [false, true] {
            let scratch = Scratch::new(if widen_directory {
                "mutated-directory"
            } else {
                "mutated-file"
            });
            let path = scratch.store();
            let store = FileStore::open(&path).expect("open");
            store
                .put(&reference(), &Secret::new(SENTINEL))
                .await
                .expect("plant prior value");
            let before = std::fs::read(&path).expect("prior file");
            let widened = if widen_directory {
                path.parent().expect("parent")
            } else {
                path.as_path()
            };
            let mode = if widen_directory { 0o755 } else { 0o644 };
            std::fs::set_permissions(widened, std::fs::Permissions::from_mode(mode))
                .expect("widen");

            let other = CredentialRef::new("tenant-b", "com.zendesk.api", "support", "api_token")
                .expect("other reference");
            let error = store
                .put(&other, &Secret::new(format!("{SENTINEL}-new")))
                .await
                .expect_err("widened metadata must refuse mutation");
            let message = error.to_string();
            assert!(!message.contains(SENTINEL), "{message}");
            assert!(!message.contains("com.zendesk.api"), "{message}");
            assert_eq!(std::fs::read(&path).expect("prior file remains"), before);
            assert!(store.stale_temporaries().is_empty());
            assert_eq!(mode_of(widened), mode, "refusal repaired unsafe metadata");

            std::fs::set_permissions(
                widened,
                std::fs::Permissions::from_mode(if widen_directory { DIR_MODE } else { FILE_MODE }),
            )
            .expect("restore");
        }
    }

    #[test]
    #[ignore = "requires euid 0 to plant a foreign-owned file; CI invokes it explicitly with sudo"]
    fn a_foreign_owned_store_is_refused_without_repair() {
        assert_eq!(
            rustix::process::geteuid().as_raw(),
            0,
            "this ignored fixture requires euid 0"
        );
        let scratch = Scratch::new("foreign-owned-store");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open as root"));
        std::fs::write(
            &path,
            format!(
                "{HEADER}\ntenants/9f3a4b2c/com.zendesk.api/support/api_token {}\n",
                hex_encode(SENTINEL.as_bytes())
            ),
        )
        .expect("plant a real entry");
        let bytes = std::fs::read(&path).expect("prior bytes");
        rustix::fs::chown(&path, Some(rustix::process::Uid::from_raw(65534)), None)
            .expect("plant foreign owner");
        let before = std::fs::symlink_metadata(&path).expect("metadata");

        let error = FileStore::open(&path).expect_err("foreign owner must be refused");
        let message = error.to_string();
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(!message.contains(SENTINEL), "{message}");
        assert!(!message.contains("com.zendesk.api"), "{message}");
        assert_eq!(std::fs::read(&path).expect("bytes remain"), bytes);
        let after = std::fs::symlink_metadata(&path).expect("metadata remains");
        assert_eq!((before.uid(), before.mode()), (after.uid(), after.mode()));
    }

    #[test]
    #[ignore = "requires euid 0 to plant a foreign-owned directory; CI invokes it explicitly with sudo"]
    fn a_foreign_owned_directory_is_refused_without_repair() {
        assert_eq!(
            rustix::process::geteuid().as_raw(),
            0,
            "this ignored fixture requires euid 0"
        );
        let scratch = Scratch::new("foreign-owned-directory");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open as root"));
        std::fs::write(
            &path,
            format!(
                "{HEADER}\ntenants/9f3a4b2c/com.zendesk.api/support/api_token {}\n",
                hex_encode(SENTINEL.as_bytes())
            ),
        )
        .expect("plant a real entry");
        let bytes = std::fs::read(&path).expect("prior bytes");
        let directory = path.parent().expect("parent");
        rustix::fs::chown(directory, Some(rustix::process::Uid::from_raw(65534)), None)
            .expect("plant foreign owner");
        let before = std::fs::symlink_metadata(directory).expect("metadata");

        let error = FileStore::open(&path).expect_err("foreign owner must be refused");
        let message = error.to_string();
        assert!(
            message.contains(&directory.display().to_string()),
            "{message}"
        );
        assert!(!message.contains(SENTINEL), "{message}");
        assert!(!message.contains("com.zendesk.api"), "{message}");
        assert_eq!(std::fs::read(&path).expect("bytes remain"), bytes);
        let after = std::fs::symlink_metadata(directory).expect("metadata remains");
        assert_eq!((before.uid(), before.mode()), (after.uid(), after.mode()));
    }

    /// Usable through the trait object, which is how a host binds it.
    #[tokio::test]
    async fn the_store_is_object_safe() {
        let scratch = Scratch::new("object-safe");
        let store: std::sync::Arc<dyn SecretStore> =
            std::sync::Arc::new(FileStore::open(scratch.store()).expect("open"));
        store
            .put(&reference(), &Secret::new(SENTINEL))
            .await
            .expect("put");
        assert_eq!(
            store.get(&reference()).await.expect("get").expose_secret(),
            SENTINEL
        );
    }

    #[test]
    fn hex_round_trips_every_byte() {
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(hex_decode(&hex_encode(&bytes)).as_deref(), Some(&bytes[..]));
        assert_eq!(hex_decode("abc"), None, "odd length");
        assert_eq!(hex_decode("zz"), None, "not hex");
        assert_eq!(hex_decode("AA"), Some(vec![0xaa]), "uppercase is read");
    }
}

#[cfg(test)]
mod portable_tests {
    use super::*;

    const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-portable-file-store";

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "connector-secrets-portable-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }

        fn store(&self) -> PathBuf {
            self.0.join("state").join("credentials")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn reference(instance: Option<&str>, leaf: &str) -> CredentialRef {
        match instance {
            Some(instance) => {
                CredentialRef::for_instance("tenant-a", "com.acme.api", instance, "default", leaf)
            }
            None => CredentialRef::new("tenant-a", "com.acme.api", "default", leaf),
        }
        .expect("valid reference")
    }

    #[tokio::test]
    async fn a_multi_credential_connection_migration_survives_restart() {
        let scratch = Scratch::new("migration-restart");
        let path = scratch.store();
        let first_instance = "0d3f79ae-b6df-4f77-8f77-438436c3b2ef";
        let second_instance = "b183db27-ec61-4d47-9783-4db28c82f4f8";
        let legacy_token = reference(None, "token");
        let legacy_signing = reference(None, "signing_secret");
        let first_token = reference(Some(first_instance), "token");
        let first_signing = reference(Some(first_instance), "signing_secret");
        let second_token = reference(Some(second_instance), "token");
        let second_signing = reference(Some(second_instance), "signing_secret");

        let store = FileStore::open(&path).expect("open");
        store
            .put(
                &legacy_token,
                &Secret::new(format!("{SENTINEL}-first-token")),
            )
            .await
            .expect("put first token");
        store
            .put(
                &legacy_signing,
                &Secret::new(format!("{SENTINEL}-first-signing")),
            )
            .await
            .expect("put first signing secret");

        let scope = CredentialScope::new("tenant-a", "com.acme.api").expect("scope");
        let mut migration = SecretBatch::new(scope.clone());
        migration
            .move_secret(legacy_token.clone(), first_token.clone())
            .expect("move first token");
        migration
            .move_secret(legacy_signing.clone(), first_signing.clone())
            .expect("move first signing secret");
        migration
            .put(
                second_token.clone(),
                Secret::new(format!("{SENTINEL}-second-token")),
            )
            .expect("put second token");
        migration
            .put(
                second_signing.clone(),
                Secret::new(format!("{SENTINEL}-second-signing")),
            )
            .expect("put second signing secret");
        store.apply(&migration).await.expect("atomic migration");
        drop(store);

        let restarted = FileStore::open(&path).expect("restart");
        assert!(restarted
            .get(&legacy_token)
            .await
            .unwrap_err()
            .is_not_found());
        assert!(restarted
            .get(&legacy_signing)
            .await
            .unwrap_err()
            .is_not_found());
        assert_eq!(
            restarted.references(&scope).await.expect("references"),
            vec![
                first_signing.clone(),
                first_token.clone(),
                second_signing.clone(),
                second_token.clone(),
            ]
        );
        for (at, expected) in [
            (first_token, format!("{SENTINEL}-first-token")),
            (first_signing, format!("{SENTINEL}-first-signing")),
            (second_token, format!("{SENTINEL}-second-token")),
            (second_signing, format!("{SENTINEL}-second-signing")),
        ] {
            assert_eq!(
                restarted
                    .get(&at)
                    .await
                    .expect("migrated value")
                    .expose_secret(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn an_injected_write_failure_preserves_the_prior_file_and_batch() {
        let scratch = Scratch::new("injected-batch-failure");
        let path = scratch.store();
        let source = reference(None, "token");
        let untouched = reference(None, "signing_secret");
        let destination = reference(Some("0d3f79ae-b6df-4f77-8f77-438436c3b2ef"), "token");
        let planted = reference(Some("b183db27-ec61-4d47-9783-4db28c82f4f8"), "token");
        let store = FileStore::open(&path).expect("open");
        store
            .put(&source, &Secret::new(format!("{SENTINEL}-source")))
            .await
            .expect("put source");
        store
            .put(&untouched, &Secret::new(format!("{SENTINEL}-untouched")))
            .await
            .expect("put untouched");
        let before = std::fs::read(&path).expect("prior file");

        let scope = CredentialScope::new("tenant-a", "com.acme.api").expect("scope");
        let mut batch = SecretBatch::new(scope);
        batch
            .move_secret(source.clone(), destination.clone())
            .expect("move");
        batch
            .put(planted.clone(), Secret::new(format!("{SENTINEL}-new")))
            .expect("put");
        store.inject_write_failure();
        let error = store
            .apply(&batch)
            .await
            .expect_err("injected failure must refuse the batch");
        let message = error.to_string();
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(!message.contains(SENTINEL), "{message}");
        assert!(!message.contains("com.acme.api"), "{message}");
        assert_eq!(std::fs::read(&path).expect("prior file remains"), before);
        assert_eq!(
            store
                .get(&source)
                .await
                .expect("source remains")
                .expose_secret(),
            format!("{SENTINEL}-source")
        );
        assert!(store.get(&destination).await.unwrap_err().is_not_found());
        assert!(store.get(&planted).await.unwrap_err().is_not_found());
        assert!(store.stale_temporaries().is_empty());
        drop(store);

        let restarted = FileStore::open(&path).expect("restart after failure");
        assert_eq!(std::fs::read(&path).expect("whole prior file"), before);
        assert_eq!(
            restarted
                .get(&source)
                .await
                .expect("source after restart")
                .expose_secret(),
            format!("{SENTINEL}-source")
        );
        assert!(restarted
            .get(&destination)
            .await
            .unwrap_err()
            .is_not_found());
        assert!(restarted.get(&planted).await.unwrap_err().is_not_found());
    }

    #[tokio::test]
    async fn bounded_writes_refuse_before_allocating_or_changing_the_prior_file() {
        let scratch = Scratch::new("bounded-write");
        let path = scratch.store();
        let store = FileStore::open(&path).expect("open");
        let before = std::fs::read(&path).expect("prior file");
        let error = store
            .put(
                &reference(None, "token"),
                &Secret::new("x".repeat(MAX_VALUE_BYTES + 1)),
            )
            .await
            .expect_err("oversized value must be refused");
        let message = error.to_string();
        assert!(message.contains(&MAX_VALUE_BYTES.to_string()), "{message}");
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(!message.contains(&"x".repeat(64)), "{message}");
        assert!(!message.contains("com.acme.api"), "{message}");
        assert_eq!(std::fs::read(&path).expect("prior file remains"), before);
        assert!(store.is_empty());
        assert!(store.stale_temporaries().is_empty());
    }

    #[test]
    fn bounded_reads_use_metadata_and_a_same_handle_max_plus_one_read() {
        let scratch = Scratch::new("bounded-read-portable");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("open"));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open planted store");
        file.set_len(MAX_FILE_BYTES as u64 + 1)
            .expect("make a sparse oversized store");
        drop(file);

        let error = FileStore::open(&path).expect_err("oversized store must be refused");
        let message = error.to_string();
        assert!(message.contains(&MAX_FILE_BYTES.to_string()), "{message}");
        assert!(message.contains(&path.display().to_string()), "{message}");

        let source = include_str!("file.rs");
        let start = source
            .find("fn read_bounded")
            .expect("bounded reader exists");
        let body = &source[start
            ..source[start..]
                .find("\nfn parse")
                .map(|end| start + end)
                .expect("bounded reader ends")];
        assert!(
            body.contains(".metadata()"),
            "metadata check left the handle"
        );
        assert!(
            body.contains("file.take(MAX_FILE_BYTES as u64 + 1)"),
            "growth-race max+1 read left the same handle"
        );
    }

    #[test]
    fn logical_v1_reads_bound_entries_and_individual_values() {
        let entries_scratch = Scratch::new("bounded-entry-count");
        let entries_path = entries_scratch.store();
        drop(FileStore::open(&entries_path).expect("open"));
        let mut contents = String::from(HEADER);
        contents.push('\n');
        for index in 0..=MAX_ENTRIES {
            contents.push_str(&format!("tenants/t{index}/com.acme.api/token 41\n"));
        }
        assert!(
            contents.len() < MAX_FILE_BYTES,
            "entry fixture must isolate its bound"
        );
        std::fs::write(&entries_path, contents).expect("plant too many entries");
        let error = FileStore::open(&entries_path).expect_err("entry count must be bounded");
        let message = error.to_string();
        assert!(message.contains(&MAX_ENTRIES.to_string()), "{message}");
        assert!(!message.contains("com.acme.api"), "{message}");

        let value_scratch = Scratch::new("bounded-value-read");
        let value_path = value_scratch.store();
        drop(FileStore::open(&value_path).expect("open"));
        let contents = format!(
            "{HEADER}\ntenants/tenant-a/com.acme.api/token {}\n",
            "41".repeat(MAX_VALUE_BYTES + 1)
        );
        assert!(
            contents.len() < MAX_FILE_BYTES,
            "value fixture must isolate its bound"
        );
        std::fs::write(&value_path, contents).expect("plant oversized encoded value");
        let error = FileStore::open(&value_path).expect_err("individual value must be bounded");
        let message = error.to_string();
        assert!(message.contains(&MAX_VALUE_BYTES.to_string()), "{message}");
        assert!(!message.contains(&"41".repeat(64)), "{message}");
    }
}

#[cfg(test)]
mod transaction_crash_tests {
    use super::*;
    use crate::{
        CredentialScope, PreparedSecretError, PreparedSecretStore, SecretProposalDigest,
        SecretTransactionGeneration, SecretTransactionId, SecretTransactionState,
    };
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    const OLD: &str = "SENTINEL-NOT-A-REAL-SECRET-crash-old";
    const NEW: &str = "SENTINEL-NOT-A-REAL-SECRET-crash-new";

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "connector-secrets-transaction-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }

        fn store(&self) -> PathBuf {
            self.0.join("store").join("credentials")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn generation() -> SecretTransactionGeneration {
        SecretTransactionGeneration::from_protocol_bytes([0, 0, 0, 0, 0, 0, 0, 1])
            .expect("non-zero")
    }

    fn id() -> SecretTransactionId {
        SecretTransactionId::new(generation(), [0x51; 24])
    }

    fn digest() -> SecretProposalDigest {
        SecretProposalDigest::from_protocol_bytes([0x52; 32])
    }

    fn reference() -> CredentialRef {
        CredentialRef::new("tenant-a", "com.acme.api", "default", "token").expect("valid")
    }

    fn batch() -> SecretBatch {
        let reference = reference();
        let mut batch = SecretBatch::new(
            CredentialScope::new(reference.tenant(), reference.authority()).expect("scope"),
        );
        batch.put(reference, Secret::new(NEW)).expect("mutation");
        batch
    }

    fn released_v0_19_1_accepts(contents: &str) -> bool {
        // This is the released v0.19.1 parser's version gate: any version line other than v1 is a
        // hard refusal. Keep the fixture small so it proves the upgrade boundary, not a second
        // implementation of the legacy store.
        !contents.lines().any(|line| {
            line.strip_prefix(VERSION_PREFIX)
                .is_some_and(|version| version != VERSION)
        })
    }

    fn run(operation: &str, store: &Path) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let store = FileStore::open(store).expect("child open");
            match operation {
                "prepare" => {
                    let _ = store.prepare(id(), digest(), &batch()).await;
                }
                "commit" => {
                    let _ = store.commit(id()).await;
                }
                "abort" => {
                    let _ = store.abort(id()).await;
                }
                "reclaim" => {
                    let _ = store.reclaim(generation()).await;
                }
                _ => panic!("unknown child operation"),
            }
        });
        panic!("the configured crash boundary was not reached");
    }

    #[test]
    fn prepared_crash_child() {
        let Ok(operation) = std::env::var("CONNECTOR_SECRETS_CRASH_CHILD") else {
            return;
        };
        let store = PathBuf::from(std::env::var_os("CONNECTOR_SECRETS_CRASH_STORE").expect("path"));
        run(&operation, &store);
    }

    #[test]
    fn lease_child() {
        let Ok(mode) = std::env::var("CONNECTOR_SECRETS_LEASE_CHILD") else {
            return;
        };
        let store = PathBuf::from(std::env::var_os("CONNECTOR_SECRETS_LEASE_STORE").expect("path"));
        match mode.as_str() {
            "hold" => {
                let _store = FileStore::open(&store).expect("holder open");
                let ready = std::env::var_os("CONNECTOR_SECRETS_LEASE_READY").expect("ready path");
                std::fs::write(ready, b"ready").expect("signal ready");
                std::thread::sleep(Duration::from_secs(30));
            }
            "probe" => match FileStore::open(&store) {
                Ok(_) => std::process::exit(0),
                Err(StoreError::Conflict { .. }) => std::process::exit(42),
                Err(_) => std::process::exit(43),
            },
            _ => panic!("unknown lease child mode"),
        }
    }

    #[test]
    fn legacy_writer_child() {
        if std::env::var_os("CONNECTOR_SECRETS_LEGACY_CHILD").is_none() {
            return;
        }
        let store = PathBuf::from(
            std::env::var_os("CONNECTOR_SECRETS_LEGACY_STORE").expect("legacy store path"),
        );
        let ready = PathBuf::from(
            std::env::var_os("CONNECTOR_SECRETS_LEGACY_READY").expect("legacy ready path"),
        );
        let release = PathBuf::from(
            std::env::var_os("CONNECTOR_SECRETS_LEGACY_RELEASE").expect("legacy release path"),
        );
        let opened = std::fs::read_to_string(&store).expect("legacy open");
        assert!(released_v0_19_1_accepts(&opened), "fixture must open v1");
        std::fs::write(&ready, b"ready").expect("signal legacy open");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !release.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(release.exists(), "legacy writer was not released");
        // Released 0.19.1 held this parsed v1 image in memory and rewrote it wholesale for every
        // point mutation. Reinstalling the bytes it opened models the unsafe stale-writer edge
        // without teaching this fixture a second credential-address encoder.
        std::fs::write(&store, opened).expect("legacy v1 rewrite");
    }

    #[test]
    fn every_durable_transaction_boundary_recovers_one_complete_state() {
        let cases: &[(&str, &[&str])] = &[
            (
                "prepare",
                &[
                    "stage:create",
                    "stage:write",
                    "stage:flush",
                    "stage:replace",
                    "stage:directory-sync",
                    "live:create",
                    "live:write",
                    "live:flush",
                    "live:replace",
                    "live:directory-sync",
                ],
            ),
            (
                "commit",
                &[
                    "live:create",
                    "live:write",
                    "live:flush",
                    "live:replace",
                    "live:directory-sync",
                    "stage:cleanup",
                ],
            ),
            (
                "abort",
                &[
                    "live:create",
                    "live:write",
                    "live:flush",
                    "live:replace",
                    "live:directory-sync",
                    "stage:cleanup",
                ],
            ),
            (
                "reclaim",
                &[
                    "live:create",
                    "live:write",
                    "live:flush",
                    "live:replace",
                    "live:directory-sync",
                ],
            ),
        ];

        for (operation, boundaries) in cases {
            for boundary in *boundaries {
                let scratch = Scratch::new(&format!(
                    "crash-{}-{}",
                    operation,
                    boundary.replace(':', "-")
                ));
                let path = scratch.store();
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .expect("runtime");
                runtime.block_on(async {
                    let store = FileStore::open(&path).expect("setup open");
                    store
                        .put(&reference(), &Secret::new(OLD))
                        .await
                        .expect("seed");
                    if matches!(*operation, "commit" | "abort") {
                        store
                            .prepare(id(), digest(), &batch())
                            .await
                            .expect("prepare");
                    } else if *operation == "reclaim" {
                        store.abort(id()).await.expect("terminal tombstone");
                    }
                });

                let status = Command::new(std::env::current_exe().expect("test executable"))
                    .arg("--exact")
                    .arg("file::transaction_crash_tests::prepared_crash_child")
                    .arg("--nocapture")
                    .env("CONNECTOR_SECRETS_CRASH_CHILD", operation)
                    .env("CONNECTOR_SECRETS_CRASH_STORE", &path)
                    .env("CONNECTOR_SECRETS_CRASH_AT", boundary)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .expect("spawn crash child");
                assert!(!status.success(), "{operation} did not crash at {boundary}");

                runtime.block_on(async {
                    let recovered = FileStore::open(&path).unwrap_or_else(|error| {
                        panic!("{operation}/{boundary} did not recover: {error}")
                    });
                    let transaction = recovered.state(id()).await;
                    let value = recovered
                        .get(&reference())
                        .await
                        .expect("one complete credential image")
                        .expose_secret()
                        .to_owned();
                    match *operation {
                        "prepare" => match transaction {
                            Ok(SecretTransactionState::Absent) => assert_eq!(value, OLD),
                            Ok(SecretTransactionState::Prepared) => {
                                assert_eq!(value, OLD);
                                recovered
                                    .commit(id())
                                    .await
                                    .expect("candidate remains committable");
                                assert_eq!(
                                    recovered
                                        .get(&reference())
                                        .await
                                        .expect("new")
                                        .expose_secret(),
                                    NEW
                                );
                            }
                            other => panic!("unexpected prepare recovery at {boundary}: {other:?}"),
                        },
                        "commit" => match transaction {
                            Ok(SecretTransactionState::Prepared) => assert_eq!(value, OLD),
                            Ok(SecretTransactionState::Committed) => assert_eq!(value, NEW),
                            other => panic!("unexpected commit recovery at {boundary}: {other:?}"),
                        },
                        "abort" => match transaction {
                            Ok(SecretTransactionState::Prepared)
                            | Ok(SecretTransactionState::Absent) => assert_eq!(value, OLD),
                            other => panic!("unexpected abort recovery at {boundary}: {other:?}"),
                        },
                        "reclaim" => match transaction {
                            Ok(SecretTransactionState::Absent)
                            | Err(PreparedSecretError::Retired) => assert_eq!(value, OLD),
                            other => panic!("unexpected reclaim recovery at {boundary}: {other:?}"),
                        },
                        _ => unreachable!(),
                    }
                });
            }
        }
    }

    #[test]
    fn two_children_prove_lease_refusal_and_abrupt_release() {
        let scratch = Scratch::new("lease-processes");
        let path = scratch.store();
        drop(FileStore::open(&path).expect("initialize"));
        let ready = scratch.0.join("holder.ready");
        let executable = std::env::current_exe().expect("test executable");
        let mut holder = Command::new(&executable)
            .arg("--exact")
            .arg("file::transaction_crash_tests::lease_child")
            .arg("--nocapture")
            .env("CONNECTOR_SECRETS_LEASE_CHILD", "hold")
            .env("CONNECTOR_SECRETS_LEASE_STORE", &path)
            .env("CONNECTOR_SECRETS_LEASE_READY", &ready)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn holder");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready.exists(), "holder did not acquire the lease");

        let probe = || {
            Command::new(&executable)
                .arg("--exact")
                .arg("file::transaction_crash_tests::lease_child")
                .arg("--nocapture")
                .env("CONNECTOR_SECRETS_LEASE_CHILD", "probe")
                .env("CONNECTOR_SECRETS_LEASE_STORE", &path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("spawn probe")
        };
        assert_eq!(probe().code(), Some(42), "contending child must refuse");
        holder.kill().expect("abruptly terminate holder");
        holder.wait().expect("reap holder");
        assert!(
            probe().success(),
            "kernel lease must release after process exit"
        );
    }

    #[test]
    fn native_upgrade_fixture_proves_legacy_quiescence_and_v2_refusal() {
        let scratch = Scratch::new("legacy-upgrade");
        let path = scratch.store();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let store = FileStore::open(&path).expect("initialize v1");
            store
                .put(&reference(), &Secret::new(OLD))
                .await
                .expect("seed legacy value");
        });

        let ready = scratch.0.join("legacy.ready");
        let release = scratch.0.join("legacy.release");
        let mut legacy = Command::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg("file::transaction_crash_tests::legacy_writer_child")
            .arg("--nocapture")
            .env("CONNECTOR_SECRETS_LEGACY_CHILD", "1")
            .env("CONNECTOR_SECRETS_LEGACY_STORE", &path)
            .env("CONNECTOR_SECRETS_LEGACY_READY", &ready)
            .env("CONNECTOR_SECRETS_LEGACY_RELEASE", &release)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn already-open legacy writer");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready.exists(), "legacy writer did not open v1");

        runtime.block_on(async {
            let current =
                FileStore::open(&path).expect("0.20 opener ignores no active legacy lock");
            assert_eq!(
                current.abort(id()).await,
                Ok(SecretTransactionState::Absent)
            );
            let v2 = std::fs::read_to_string(&path).expect("read migrated v2");
            assert!(
                !released_v0_19_1_accepts(&v2),
                "a fresh released v0.19.1 parser must refuse v2"
            );

            std::fs::write(&release, b"rewrite").expect("release legacy writer");
            assert!(legacy.wait().expect("wait for legacy writer").success());
            assert_eq!(
                current.state(id()).await,
                Ok(SecretTransactionState::Absent),
                "an already-open legacy writer can erase the acknowledged tombstone"
            );
        });
        assert!(released_v0_19_1_accepts(
            &std::fs::read_to_string(&path).expect("legacy v1 survived")
        ));
    }
}
