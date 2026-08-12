//! Reading inputs and writing artifacts.
//!
//! Generated artifacts are committed and reviewed, so a half-written file is worse than no file: it
//! is a diff a human might approve. Two rules keep that from happening.
//!
//! 1. **Per file**: write to a temporary in the same directory, then rename. A rename within a
//!    directory is atomic, so a reader sees the old bytes or the new ones and never a prefix.
//! 2. **Per run**: [`crate::pipeline`] compiles *every* provider before writing *any* file, so a
//!    provider that fails to compile aborts the run with the tree untouched.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, Result};

/// Read a UTF-8 file, naming it on failure.
pub fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

/// Read a UTF-8 file, or `None` when it does not exist.
///
/// Distinguishes "absent" from "unreadable": a permissions error must not be reported as a new
/// artifact, which would make `build` silently overwrite a file it could not inspect.
pub fn read_if_exists(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Write `contents` to `path`, atomically with respect to any concurrent reader.
pub fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;

    let temporary = temporary_path(path);
    write_then_rename(&temporary, path, contents).inspect_err(|_| {
        // Best effort: a failed write must not leave debris next to the artifact either.
        let _ = fs::remove_file(&temporary);
    })
}

fn write_then_rename(temporary: &Path, path: &Path, contents: &str) -> Result<()> {
    let mut file = fs::File::create(temporary)
        .with_context(|| format!("cannot create {}", temporary.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    // Flush to the device before the rename, so a crash cannot publish an empty file.
    file.sync_all()
        .with_context(|| format!("cannot flush {}", temporary.display()))?;
    drop(file);

    fs::rename(temporary, path).with_context(|| format!("cannot replace {}", path.display()))
}

/// A sibling temporary name — same directory, so the rename stays within one filesystem.
fn temporary_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "artifact".to_string());
    let parent = path.parent().unwrap_or(Path::new("."));
    parent.join(format!(".{name}.tmp-{}-{unique}", std::process::id()))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::panic;
    use std::sync::{Arc, Mutex};

    use super::*;

    /// Fixture directories for `label` and this process still sitting in `root`.
    ///
    /// Scoped to one label *and* this process id on purpose. Sibling tests run concurrently and own
    /// live fixtures of their own, and another agent's run owns its own debris; a scan that tripped
    /// over either would be exactly as untrustworthy as the flake it exists to prevent.
    fn surviving_fixtures(root: &Path, label: &str) -> Vec<PathBuf> {
        let prefix = format!("flux-connectors-artifact-{label}-{}", std::process::id());
        let Ok(entries) = fs::read_dir(root) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
            })
            .collect()
    }

    /// Cleanup is not "the last line of a passing test": a panicking test must remove its fixture
    /// too. Skipping that is what left 55 `flux-connectors-artifact-*` directories in a tmpfs that
    /// was 80% full, and made a full `cargo test --workspace` fail twice under a wave of concurrent
    /// builds — both times blamed on the diff that happened to be merging.
    #[test]
    fn a_fixture_does_not_survive_a_panicking_test() {
        let recorded = Arc::new(Mutex::new(None));
        let inside = Arc::clone(&recorded);

        // The panic is the point, so the default hook prints it. Leave it: replacing the hook is
        // process-global and would race the tests running beside this one.
        let outcome = panic::catch_unwind(move || {
            let dir = scratch("panic");
            *inside.lock().unwrap() = Some(dir.to_path_buf());
            write_atomic(&dir.join("zendesk.flux"), "ops\n").unwrap();
            panic!("a failing assertion, as a real test would");
        });
        assert!(outcome.is_err(), "the closure was supposed to panic");

        let recorded = recorded
            .lock()
            .unwrap()
            .clone()
            .expect("the closure built a fixture before panicking");
        let root = recorded
            .parent()
            .expect("a fixture directory has a parent directory");
        let survivors = surviving_fixtures(root, "panic");
        assert!(
            survivors.is_empty(),
            "fixtures survived a panicking test: {survivors:?}"
        );
    }

    /// Where the fixtures live, and that no two of them ever share a path.
    ///
    /// `env::temp_dir()` is shared by every concurrent agent on this machine and bounded by a
    /// tmpfs; the build directory is per-worktree and already git-ignored. A label plus a process id
    /// is not unique either — a pid is reused, and two agents running the same binary collide.
    #[test]
    fn fixtures_live_in_the_build_directory_and_never_repeat_a_path() {
        let first = scratch("unique");
        let second = scratch("unique");
        assert_ne!(
            first.to_path_buf(),
            second.to_path_buf(),
            "two fixtures with the same label share a path, so one clobbers the other"
        );

        // The test binary sits in `<build>/<profile>/deps/`, so its own path names the build
        // directory the fixtures must stay inside.
        let build_dir = std::env::current_exe()
            .expect("the test binary knows its own path")
            .parent()
            .and_then(Path::parent)
            .expect("the test binary sits in <build>/<profile>/deps")
            .to_path_buf();
        for dir in [first.to_path_buf(), second.to_path_buf()] {
            assert!(
                dir.starts_with(&build_dir),
                "{} is outside the build directory {}",
                dir.display(),
                build_dir.display()
            );
            assert!(
                !dir.starts_with(std::env::temp_dir()),
                "{} is in the shared temporary directory",
                dir.display()
            );
        }
    }

    /// A fixture directory that removes itself, wherever the test that owns it stops.
    ///
    /// It stands in for a `PathBuf` — same `Deref` and `AsRef<Path>` impls — so a test body reads
    /// exactly as it did before the guard existed.
    ///
    /// Reachable from the crate's other unit tests (`pipeline`'s, C-544) rather than copied into
    /// them: where a fixture lives is one rule with two spellings already — this one and
    /// `tests/common/mod.rs`'s, which `tests/main/fixture_hygiene.rs` holds to the same properties —
    /// and a third would be the drift both of those docstrings warn about.
    pub(crate) struct Scratch {
        path: PathBuf,
    }

    impl std::ops::Deref for Scratch {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.path
        }
    }

    impl AsRef<Path> for Scratch {
        fn as_ref(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // Best effort, and deliberately not asserted: a `Drop` that panics while the thread is
            // already unwinding from a failed assertion aborts the process and buries the real
            // failure.
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// A fixture directory under the build's own tree, removed when `Scratch` is dropped.
    ///
    /// Both properties were learned from a flake that cost two wrong diagnoses (C-143).
    ///
    /// 1. **Not `env::temp_dir()`.** That is a bounded tmpfs every concurrent agent on the machine
    ///    writes to; filling it turns a write into a test failure with nothing to do with the code
    ///    under test. The build directory is per-worktree and already git-ignored.
    /// 2. **Removed on every path, including a panic** — hence a `Drop` impl rather than a call at
    ///    the end of each test, which only runs when the test passes.
    pub(crate) fn scratch(label: &str) -> Scratch {
        // The test binary is `<build>/<profile>/deps/<binary>`, so its own path names the build
        // directory. Deriving it this way follows `CARGO_TARGET_DIR` instead of assuming a layout.
        let root = std::env::current_exe()
            .expect("the test binary knows its own path")
            .parent()
            .and_then(Path::parent)
            .expect("the test binary sits in <build>/<profile>/deps")
            .join("artifact-fixtures");

        let path = root.join(format!(
            "flux-connectors-artifact-{label}-{}-{}",
            std::process::id(),
            run_unique()
        ));
        fs::create_dir_all(&path).expect("create scratch dir");
        Scratch { path }
    }

    /// A component no other run and no other call repeats.
    ///
    /// A process id alone is not enough: the kernel recycles them, and two agents running two copies
    /// of this binary can hold the same one at the same time.
    fn run_unique() -> String {
        static RUN: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
        static CALL: AtomicU32 = AtomicU32::new(0);

        let run = RUN.get_or_init(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since_epoch| since_epoch.as_nanos())
        });
        format!("{run:x}-{}", CALL.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn write_atomic_creates_missing_directories() {
        let dir = scratch("create");
        let path = dir.join("nested/zendesk.flux");
        write_atomic(&path, "ops\n").unwrap();
        assert_eq!(read(&path).unwrap(), "ops\n");
    }

    #[test]
    fn write_atomic_replaces_and_leaves_no_temporary() {
        let dir = scratch("replace");
        let path = dir.join("zendesk.flux");
        write_atomic(&path, "old\n").unwrap();
        write_atomic(&path, "new\n").unwrap();

        assert_eq!(read(&path).unwrap(), "new\n");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temporary files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn read_if_exists_distinguishes_absent_from_present() {
        let dir = scratch("absent");
        assert!(read_if_exists(&dir.join("missing.flux")).unwrap().is_none());
        write_atomic(&dir.join("there.flux"), "x").unwrap();
        assert_eq!(
            read_if_exists(&dir.join("there.flux")).unwrap().as_deref(),
            Some("x")
        );
    }
}
