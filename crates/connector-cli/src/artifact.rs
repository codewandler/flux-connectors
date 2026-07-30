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
mod tests {
    use super::*;

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "flux-connectors-artifact-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn write_atomic_creates_missing_directories() {
        let dir = scratch("create");
        let path = dir.join("nested/zendesk.flux");
        write_atomic(&path, "ops\n").unwrap();
        assert_eq!(read(&path).unwrap(), "ops\n");
        fs::remove_dir_all(&dir).unwrap();
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
        fs::remove_dir_all(&dir).unwrap();
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
        fs::remove_dir_all(&dir).unwrap();
    }
}
