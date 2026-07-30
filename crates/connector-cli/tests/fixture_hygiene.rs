//! Where the integration fixtures live, and that none of them outlives the test that owns it.
//!
//! Every integration binary in this crate builds its trees through [`common::Fixture`], so a
//! defect in that harness is not one test's problem — it is the whole gate's. It has been measured
//! twice (C-143, C-150): the fixture root used to be built under `std::env::temp_dir()`, which on
//! this machine is a **32 GB tmpfs shared by every concurrent agent**, under a name keyed on the
//! process id and a process-local counter. Two agents running the same binary do not get different
//! names from that, and with the tmpfs exhausted `fs::write` inside the harness fails — taking down
//! `wiring`, `no_network`, `service_units` and `site_catalog` at once, in output that reads exactly
//! like a merge regression. Twice the first hypothesis was "the merge broke it", and once a good
//! merge was reverted before the cause was measured.
//!
//! So the harness's hygiene is asserted, not assumed. A flaky integration gate is worse than a
//! missing one, because it teaches a coordinator to distrust a red gate.
//!
//! # Every scan here is scoped by label *and* process id
//!
//! Deliberately, and this is C-143's finding rather than caution: sibling tests run concurrently in
//! this same process and own live fixtures of their own, and another agent's worktree owns its own
//! debris — `/tmp/flux-connectors-artifact-{create,replace,absent}-664560` was sitting there while
//! C-143 ran. An unscoped scan of `/tmp` fails on either, which would make this file exactly as
//! untrustworthy as the flake it exists to prevent.

mod common;

use std::panic;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use common::Fixture;

/// The prefix every fixture directory of `label`, in *this* process, shares.
fn prefix(label: &str) -> String {
    format!("flux-connectors-{label}-{}", std::process::id())
}

/// Fixture directories for `label` and this process currently sitting in `dir`.
fn fixtures_in(dir: &Path, label: &str) -> Vec<PathBuf> {
    let prefix = prefix(label);
    let Ok(entries) = std::fs::read_dir(dir) else {
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

/// The build directory this test binary was compiled into.
///
/// The binary is `<build>/<profile>/deps/<name>-<hash>`, so its own path names the profile
/// directory. Deriving it this way follows `CARGO_TARGET_DIR` instead of assuming a layout — the
/// same derivation `artifact.rs` uses, because two spellings of one rule drift apart.
fn build_dir() -> PathBuf {
    std::env::current_exe()
        .expect("the test binary knows its own path")
        .parent()
        .and_then(Path::parent)
        .expect("the test binary sits in <build>/<profile>/deps")
        .to_path_buf()
}

/// A live fixture does not put a byte in the shared temporary directory.
///
/// This is the leak assertion. It is stated over a *live* fixture on purpose: the harness already
/// removes its tree on drop, so what made the gate untrustworthy was never a missing `Drop` — it
/// was that the tree sat in a bounded tmpfs every other agent on the machine was also filling. The
/// survivors this names are the fixture roots themselves.
#[test]
fn a_fixture_never_occupies_the_shared_temporary_directory() {
    let label = "hygiene-shared-tmp";
    let fixture = Fixture::with_provider(label, "acme");

    let debris = fixtures_in(&std::env::temp_dir(), label);
    assert!(
        debris.is_empty(),
        "fixtures occupy the shared temporary directory {}, \
         which is a bounded tmpfs every concurrent agent writes to: {debris:?}",
        std::env::temp_dir().display()
    );

    // Keep the fixture alive across the scan; the point is where it lives, not that it is cleaned.
    drop(fixture);
}

/// A fixture lives under the build's own tree, which is per-worktree and already git-ignored.
#[test]
fn a_fixture_lives_in_the_build_directory() {
    let fixture = Fixture::with_provider("hygiene-build-dir", "acme");
    let root = fixture.root();
    let build_dir = build_dir();

    assert!(
        root.starts_with(&build_dir),
        "{} is outside the build directory {}",
        root.display(),
        build_dir.display()
    );
    assert!(
        !root.starts_with(std::env::temp_dir()),
        "{} is in the shared temporary directory",
        root.display()
    );
}

/// A fixture is removed on **every** path, including when the test that owns it panics.
///
/// Cleanup is not "the last line of a passing test": the tests that matter here are the failing
/// ones, and a fixture left behind by a failed assertion is exactly the debris that fills the disk
/// for the next run.
#[test]
fn a_fixture_does_not_survive_a_panicking_test() {
    let label = "hygiene-panic";
    let recorded = Arc::new(Mutex::new(None));
    let inside = Arc::clone(&recorded);

    // The panic is the point, so the default hook prints it. Leave the hook alone: replacing it is
    // process-global and would race the tests running beside this one.
    let outcome = panic::catch_unwind(move || {
        let fixture = Fixture::with_provider(label, "acme");
        *inside.lock().unwrap() = Some(fixture.root().to_path_buf());
        fixture.write("connectors/acme.flux", "ops\n");
        panic!("a failing assertion, as a real test would");
    });
    assert!(outcome.is_err(), "the closure was supposed to panic");

    let recorded = recorded
        .lock()
        .unwrap()
        .clone()
        .expect("the closure built a fixture before panicking");
    let parent = recorded
        .parent()
        .expect("a fixture root has a parent directory");
    let survivors = fixtures_in(parent, label);
    assert!(
        survivors.is_empty(),
        "fixtures survived a panicking test: {survivors:?}"
    );
}

/// A fixture path repeats neither within a run nor across runs.
///
/// The negative half is the measured collision, spelled out rather than described: a name of
/// `flux-connectors-{label}-{pid}-{counter}` is reproduced exactly by a second run of the same
/// binary that happens to hold a recycled pid, which is how two agents collided in one wave. The
/// forbidden set is the whole of the old scheme, so this says "not that" without prescribing which
/// run-scoped component replaces it.
#[test]
fn a_fixture_path_is_unique_per_test_and_per_run() {
    let label = "hygiene-unique";
    let first = Fixture::new(label);
    let second = Fixture::new(label);
    assert_ne!(
        first.root(),
        second.root(),
        "two fixtures with the same label share a path, so one clobbers the other"
    );

    let reproducible: Vec<String> = (0..64)
        .map(|call| format!("{}-{call}", prefix(label)))
        .collect();
    for root in [first.root(), second.root()] {
        let leaf = root
            .file_name()
            .expect("a fixture root has a directory name")
            .to_string_lossy()
            .into_owned();
        assert!(
            !reproducible.contains(&leaf),
            "{leaf} is a process id plus a process-local counter, \
             which a second run of this binary can reproduce exactly"
        );
    }
}
