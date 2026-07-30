//! Fixture scaffolding shared by the `connector-cli` integration tests.
//!
//! Deliberately dependency-free: the crate has no `tempfile`, and adding one would collide with the
//! other connector stories in flight.

// Each integration-test binary includes this module and uses a different part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A throwaway directory tree, removed when the value drops.
pub struct Fixture {
    root: PathBuf,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

impl Fixture {
    /// Create an empty fixture root under the system temp directory.
    pub fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "flux-connectors-{label}-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create fixture root");
        Self { root }
    }

    /// A fixture with one provider definition and one vendored spec, the shape a build expects.
    pub fn with_provider(label: &str, provider: &str) -> Self {
        let fixture = Self::new(label);
        fixture.write_provider(provider, "id = \"acme\"\n");
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
        let _ = fs::remove_dir_all(&self.root);
    }
}
