//! Fixture scaffolding shared by the `connector-cli` integration tests.
//!
//! Deliberately dependency-free: the crate has no `tempfile`, and adding one would collide with the
//! other connector stories in flight.
//!
//! # Fixtures live in the build tree, not in `/tmp`
//!
//! Every integration binary in this crate builds its trees here, so where they land is a property
//! of the whole gate rather than of one test. It used to be `std::env::temp_dir()`, keyed on the
//! process id and a process-local counter, and that cost two wrong diagnoses (C-143, C-150): `/tmp`
//! on the development machine is a **32 GB tmpfs shared by every concurrent agent**, and a pid plus
//! a process-local counter does not separate two agents running the same binary. With the tmpfs
//! exhausted, [`Fixture::write`]'s `fs::write` fails and takes `wiring`, `no_network`,
//! `service_units` and `site_catalog` down together — output that reads exactly like a merge
//! regression, and twice it was read as one.
//!
//! So a fixture root is now derived from `current_exe()`, which follows `CARGO_TARGET_DIR` and is
//! per-worktree and already git-ignored, under a name no other run repeats. This is the same fix
//! `src/artifact.rs`'s test `scratch()` carries; the two are deliberately one shape, because two
//! spellings of one rule drift apart. `tests/fixture_hygiene.rs` asserts both halves.

// Each integration-test binary includes this module and uses a different part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

/// A complete hand-authored provider definition for `id`, of the shape the real loader accepts and
/// the real emitter can compile: one GET with a path parameter.
///
/// The fixtures used to be `id = "acme"` and nothing more, which was all C-13's placeholder loader
/// looked at. `connector-spec`'s loader validates for real (C-27), so a fixture now has to be a
/// connector rather than a stand-in for one.
///
/// The operation id is kebab, not dotted: a dotted name cannot be a Flux `op` **declaration** name
/// and the emitter refuses one rather than rewriting it (C-23 decides the public form).
pub fn definition(id: &str) -> String {
    format!(
        r#"id = "{id}"
vendor = "{id} Inc."
base_url = "https://api.{id}.example"
description = "A hand-authored fixture connector."

[[operations]]
id = "{id}-thing-get"
method = "GET"
direction = "read"
path = "/v1/things/{{thing_id}}"
description = "Fetch one thing."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "thing_id"
description = "The thing to fetch."
required = true
schema = {{ type = "integer" }}
"#
    )
}

/// A throwaway directory tree, removed when the value drops.
pub struct Fixture {
    root: PathBuf,
}

/// A path component that neither another run nor another call in this run repeats.
///
/// The run half is the wall clock at first use, read once per process; the call half is a counter.
/// A process id is not a substitute for either: the kernel recycles them, and two agents running
/// two copies of this binary can hold the same one at the same time — which is precisely how two
/// agents collided in one wave, on fixtures neither of their diffs could reach.
fn run_unique() -> String {
    static RUN: OnceLock<u128> = OnceLock::new();
    static CALL: AtomicU32 = AtomicU32::new(0);

    let run = RUN.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since_epoch| since_epoch.as_nanos())
    });
    format!("{run:x}-{}", CALL.fetch_add(1, Ordering::Relaxed))
}

impl Fixture {
    /// Create an empty fixture root under the build's own tree.
    ///
    /// Two properties, both learned from a flake that cost two wrong diagnoses (C-143, C-150) and
    /// both asserted by `tests/fixture_hygiene.rs`:
    ///
    /// 1. **Not `env::temp_dir()`.** That is a bounded tmpfs every concurrent agent on the machine
    ///    writes to; filling it turns a fixture write into a failure in four integration binaries
    ///    that have nothing to do with the code under test. The build directory is per-worktree.
    /// 2. **A name no other run repeats.** A pid is recycled by the kernel and shared by two agents
    ///    running two copies of this binary, so a pid plus a process-local counter — the old scheme
    ///    — is exactly reproducible by a second run. See [`run_unique`].
    pub fn new(label: &str) -> Self {
        // The test binary is `<build>/<profile>/deps/<binary>`, so its own path names the profile
        // directory. Deriving it this way follows `CARGO_TARGET_DIR` instead of assuming a layout.
        let fixtures = std::env::current_exe()
            .expect("the test binary knows its own path")
            .parent()
            .and_then(Path::parent)
            .expect("the test binary sits in <build>/<profile>/deps")
            .join("integration-fixtures");

        let root = fixtures.join(format!(
            "flux-connectors-{label}-{}-{}",
            std::process::id(),
            run_unique()
        ));
        fs::create_dir_all(&root).expect("create fixture root");
        Self { root }
    }

    /// A fixture with one provider definition and one vendored spec, the shape a build expects.
    pub fn with_provider(label: &str, provider: &str) -> Self {
        let fixture = Self::new(label);
        fixture.write_provider(provider, &definition(provider));
        fixture.write_spec(provider, "v1", "{\"openapi\":\"3.0.3\"}\n");
        fixture
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent");
        }
        fs::write(&path, contents).expect("write fixture file");
    }

    pub fn write_provider(&self, provider: &str, contents: &str) {
        self.write(&format!("providers/{provider}.toml"), contents);
    }

    pub fn write_spec(&self, provider: &str, version: &str, contents: &str) {
        self.write(&format!("specs/{provider}/{version}.json"), contents);
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).expect("read fixture file")
    }

    pub fn exists(&self, relative: &str) -> bool {
        self.root.join(relative).exists()
    }

    /// Every file under the fixture root, mapped to its exact bytes.
    ///
    /// Comparing two snapshots is how the tests prove `diff` wrote nothing and that a rebuild is
    /// byte-identical — both are whole-tree properties, not per-file ones.
    pub fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut files = BTreeMap::new();
        collect(&self.root, &self.root, &mut files);
        files
    }
}

fn collect(root: &Path, dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
    let entries = fs::read_dir(dir).expect("read fixture dir");
    for entry in entries {
        let entry = entry.expect("fixture dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, into);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("relative path")
                .to_path_buf();
            into.insert(relative, fs::read(&path).expect("read fixture file"));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Every path, including a panicking test: the tests that matter here are the failing ones,
        // and a tree left behind by a failed assertion is the debris that fills the disk for the
        // next run. Best effort, and deliberately not asserted — a `Drop` that panics while the
        // thread is already unwinding from a failed assertion aborts the process and buries the
        // real failure.
        let _ = fs::remove_dir_all(&self.root);
    }
}
