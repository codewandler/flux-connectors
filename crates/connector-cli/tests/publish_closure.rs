//! Publishing is the one irreversible thing this repository can do, so the order it happens in and
//! the metadata it burns are asserted here rather than trusted.
//!
//! Three properties, each of which has a way of being wrong that only shows up at release time:
//!
//! 1. **The closure is the closure.** C-190 names three consumable crates —
//!    `connector-catalog`, `connector-secrets`, `connector-pack` — but `connector-secrets`
//!    re-exports `CredentialRef` from `connector-spec`, so `connector-spec` is published too or
//!    `connector-secrets` does not resolve for anyone outside this workspace. The set is computed
//!    from the manifests, not listed, so a new edge grows the closure instead of silently breaking
//!    the next release.
//! 2. **The order is a topological sort.** crates.io refuses a crate whose dependencies are not yet
//!    live, and a half-published closure cannot be rolled back. `scripts/publish-crates-io.sh`
//!    derives its order from `cargo metadata`; this test recomputes it independently from the
//!    manifests and requires the two to agree.
//! 3. **The metadata is complete.** `description`, `license`, `repository`, `readme` and `keywords`
//!    are what a crates.io page is made of, and none of them can be corrected in a version that has
//!    already been published — only in the next one.
//!
//! The packaging itself (files excluded, a `readme` that points nowhere, a dependency without a
//! version) is proved by `cargo publish --dry-run` in the `package` job of
//! `.github/workflows/ci.yml`. That needs the registry and a full verify build, so it belongs in CI
//! rather than here; this test is the part that runs in the ordinary gate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The crates a consumer is meant to add — the roots of the publish closure. This list is a
/// *policy* choice (C-190 decides what is consumable); everything below it is derived.
///
/// Keep in sync with `ROOTS` in `scripts/publish-crates-io.sh`; the script is the executable copy
/// and `roots_match_the_script` asserts they have not drifted.
const ROOTS: &[&str] = &[
    "codewandler-connector-catalog",
    "codewandler-connector-secrets",
    "codewandler-connector-pack",
];

/// Metadata every published crate must carry. Each of these is either required by crates.io or
/// impossible to add to a version after the fact.
const REQUIRED_FIELDS: &[&str] = &["description", "license", "repository", "readme", "keywords"];

/// The publish script, relative to the workspace root.
const SCRIPT: &str = "scripts/publish-crates-io.sh";

/// Acceptance: "Publish **order** lives in `scripts/publish-crates-io.sh` and is derived from the
/// actual dependency graph, not hand-listed."
///
/// The script computes its order at run time; this recomputes it from the manifests by a different
/// route and requires agreement. Two implementations of the same topological sort disagreeing is a
/// signal either way — a hand-edit to the script, or an edge nobody noticed.
#[test]
fn the_script_publishes_the_derived_closure_in_dependency_order() {
    let workspace = Workspace::read();
    let expected = workspace.publish_order(ROOTS);
    let actual = script_order();

    assert_eq!(
        actual, expected,
        "`{SCRIPT} --print-order` disagrees with the order derived from the manifests.\n\
         script:   {actual:?}\n\
         manifests: {expected:?}\n\
         The script derives its order from `cargo metadata`, so a disagreement means the graph \
         changed under one of them."
    );
}

/// The order is only worth asserting if it is actually a valid publish order, so state that
/// property directly rather than inferring it from the equality above.
#[test]
fn every_crate_is_published_after_everything_it_depends_on() {
    let workspace = Workspace::read();
    let order = script_order();

    let mut published: BTreeSet<&str> = BTreeSet::new();
    for name in &order {
        for dependency in workspace.workspace_dependencies(name) {
            assert!(
                published.contains(dependency.as_str()),
                "`{name}` is published before `{dependency}`, which it depends on. crates.io \
                 rejects a crate whose dependencies are not already live, and the crates published \
                 before the failure cannot be withdrawn.\norder: {order:?}"
            );
        }
        published.insert(name);
    }
}

/// Acceptance: the closure is derived, and the derivation is what surfaces that it is larger than
/// the three crates C-190 asked for.
#[test]
fn the_closure_contains_every_crate_the_roots_reach() {
    let workspace = Workspace::read();
    let order: BTreeSet<String> = script_order().into_iter().collect();

    for root in ROOTS {
        assert!(
            order.contains(*root),
            "`{root}` is a consumable crate but is not in the publish order: {order:?}"
        );
        for dependency in workspace.closure(root) {
            assert!(
                order.contains(&dependency),
                "`{root}` depends on `{dependency}` (directly or transitively), but `{dependency}` \
                 is not published. A consumer outside this workspace cannot resolve `{root}` \
                 without it — the path dependency that makes it work here does not travel."
            );
        }
    }
}

/// Acceptance: "Every published crate carries the metadata crates.io requires and this repo's own
/// conventions want: `description`, `license`, `repository`, `readme`, `keywords`."
///
/// Checked over the *derived* closure rather than a list, so a crate that joins the closure later
/// cannot join it without its metadata.
#[test]
fn every_published_crate_carries_its_metadata() {
    let workspace = Workspace::read();
    let mut missing: Vec<String> = Vec::new();

    for name in workspace.publish_order(ROOTS) {
        let manifest = workspace.manifest(&name);
        for field in REQUIRED_FIELDS {
            if !manifest.declares(field) {
                missing.push(format!("{name}: {field}"));
            }
        }
        // A `readme` that names a file which is not there packages a broken crates.io page, and
        // `cargo publish` does not always object.
        if let Some(readme) = manifest.readme() {
            let path = workspace.directory(&name).join(&readme);
            if !path.exists() {
                missing.push(format!("{name}: readme `{readme}` does not exist"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "published crates are missing metadata that cannot be corrected once a version is live:\n  \
         {}\nSet it in the crate's `[package]` table (or inherit it with `field.workspace = true`).",
        missing.join("\n  ")
    );
}

/// The script's `ROOTS` and this test's must be the same list, or one of them is asserting about a
/// closure the other does not publish.
#[test]
fn roots_match_the_script() {
    let text = std::fs::read_to_string(workspace_root().join(SCRIPT))
        .unwrap_or_else(|error| panic!("read {SCRIPT}: {error}"));

    // `ROOTS=(` … `)` — one crate name per line, comments and blanks ignored.
    let body = text
        .split_once("ROOTS=(")
        .unwrap_or_else(|| panic!("{SCRIPT} has no `ROOTS=(` array"))
        .1
        .split_once(')')
        .unwrap_or_else(|| panic!("{SCRIPT}'s `ROOTS=(` array is not closed"))
        .0;
    let declared: Vec<&str> = body
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .collect();

    assert_eq!(
        declared, ROOTS,
        "{SCRIPT}'s ROOTS and this test's ROOTS have drifted"
    );
}

/// The topological sort itself, asserted over a stated graph. The real graph has four crates and
/// one non-trivial edge, so it would go on passing if the walk stopped looking after the first
/// level.
#[test]
fn the_sort_orders_a_dependency_two_edges_away() {
    let workspace = Workspace::over(&[
        ("leaf", &[]),
        ("middle", &["leaf"]),
        ("top", &["middle"]),
        ("unrelated", &[]),
    ]);

    let order = workspace.publish_order(&["top"]);
    assert_eq!(order, vec!["leaf", "middle", "top"]);

    // A second root joins the closure; a crate outside it stays out.
    let order = workspace.publish_order(&["top", "unrelated"]);
    assert_eq!(order, vec!["leaf", "middle", "top", "unrelated"]);
    assert_eq!(workspace.publish_order(&["middle"]), vec!["leaf", "middle"]);
}

/// What `scripts/publish-crates-io.sh --print-order` reports: the crates it would publish, in the
/// order it would publish them.
fn script_order() -> Vec<String> {
    let root = workspace_root();
    let script = root.join(SCRIPT);
    assert!(
        script.exists(),
        "{SCRIPT} does not exist; the publish order has nowhere to live"
    );

    let output = Command::new("bash")
        .arg(&script)
        .arg("--print-order")
        .current_dir(&root)
        .output()
        .unwrap_or_else(|error| panic!("run {SCRIPT}: {error}"));
    assert!(
        output.status.success(),
        "{SCRIPT} --print-order failed ({}):\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The workspace's own crates and the edges between them, read from the manifests.
///
/// Manifests rather than `Cargo.lock`: the lock records the resolved graph of *everything*, and the
/// question here is only which crates in this workspace depend on which others.
struct Workspace {
    /// Crate name to its directory, relative to the workspace root.
    directories: BTreeMap<String, PathBuf>,
    /// Crate name to the workspace crates it depends on, in any dependency table.
    edges: BTreeMap<String, BTreeSet<String>>,
    /// Crate name to its parsed manifest.
    manifests: BTreeMap<String, Manifest>,
}

impl Workspace {
    /// Read every member of the root workspace.
    ///
    /// # Panics
    ///
    /// If the root manifest or a member manifest cannot be read or parsed — in a test each of those
    /// is the assertion failing rather than a condition to recover from.
    fn read() -> Self {
        let root = workspace_root();
        let text = std::fs::read_to_string(root.join("Cargo.toml"))
            .unwrap_or_else(|error| panic!("read the workspace manifest: {error}"));
        let document: toml::Value = text
            .parse()
            .unwrap_or_else(|error| panic!("parse the workspace manifest: {error}"));

        let members = document
            .get("workspace")
            .and_then(|workspace| workspace.get("members"))
            .and_then(toml::Value::as_array)
            .expect("the root manifest has `[workspace] members`");

        let mut directories = BTreeMap::new();
        let mut manifests = BTreeMap::new();
        for member in members {
            let relative = member
                .as_str()
                .expect("a workspace member is a path string");
            let path = root.join(relative).join("Cargo.toml");
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let manifest = Manifest::parse(&text, &path);
            directories.insert(manifest.name.clone(), PathBuf::from(relative));
            manifests.insert(manifest.name.clone(), manifest);
        }

        // An edge only counts if both ends are members: a registry dependency is already published.
        let names: BTreeSet<&String> = manifests.keys().collect();
        let edges = manifests
            .iter()
            .map(|(name, manifest)| {
                let internal = manifest
                    .dependencies
                    .iter()
                    .filter(|dependency| names.contains(dependency))
                    .cloned()
                    .collect();
                (name.clone(), internal)
            })
            .collect();

        Self {
            directories,
            edges,
            manifests,
        }
    }

    /// A graph stated directly, for asserting the sort itself.
    fn over(edges: &[(&str, &[&str])]) -> Self {
        Self {
            directories: BTreeMap::new(),
            edges: edges
                .iter()
                .map(|(name, dependencies)| {
                    (
                        (*name).to_owned(),
                        dependencies.iter().map(|d| (*d).to_owned()).collect(),
                    )
                })
                .collect(),
            manifests: BTreeMap::new(),
        }
    }

    /// The workspace crates `name` depends on directly.
    fn workspace_dependencies(&self, name: &str) -> BTreeSet<String> {
        self.edges.get(name).cloned().unwrap_or_default()
    }

    /// Every workspace crate reachable from `root`, `root` excluded.
    fn closure(&self, root: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut pending = vec![root.to_owned()];
        while let Some(name) = pending.pop() {
            for dependency in self.edges.get(&name).into_iter().flatten() {
                if seen.insert(dependency.clone()) {
                    pending.push(dependency.clone());
                }
            }
        }
        seen
    }

    /// The roots and everything they reach, in an order where a crate always follows its
    /// dependencies. Kahn's algorithm with ties broken by name, so the order is deterministic and
    /// two runs of the same graph are comparable.
    fn publish_order(&self, roots: &[&str]) -> Vec<String> {
        let mut closure: BTreeSet<String> = roots.iter().map(|root| (*root).to_owned()).collect();
        for root in roots {
            closure.extend(self.closure(root));
        }

        let mut ordered: Vec<String> = Vec::with_capacity(closure.len());
        let mut placed: BTreeSet<String> = BTreeSet::new();
        while placed.len() < closure.len() {
            // The lowest-named crate whose dependencies are all placed.
            let next = closure
                .iter()
                .find(|name| {
                    !placed.contains(*name)
                        && self
                            .workspace_dependencies(name)
                            .iter()
                            .filter(|dependency| closure.contains(*dependency))
                            .all(|dependency| placed.contains(dependency))
                })
                .unwrap_or_else(|| {
                    panic!("the workspace dependency graph has a cycle among {closure:?}")
                })
                .clone();
            placed.insert(next.clone());
            ordered.push(next);
        }
        ordered
    }

    /// The parsed manifest of a workspace member.
    fn manifest(&self, name: &str) -> &Manifest {
        self.manifests
            .get(name)
            .unwrap_or_else(|| panic!("`{name}` is not a member of this workspace"))
    }

    /// A member's directory, absolute.
    fn directory(&self, name: &str) -> PathBuf {
        workspace_root().join(
            self.directories
                .get(name)
                .unwrap_or_else(|| panic!("`{name}` is not a member of this workspace")),
        )
    }
}

/// One member's `Cargo.toml`, reduced to what publishing cares about.
struct Manifest {
    name: String,
    /// Every dependency named in any dependency table, including target- and dev-specific ones.
    dependencies: BTreeSet<String>,
    package: toml::Value,
}

impl Manifest {
    fn parse(text: &str, path: &Path) -> Self {
        let document: toml::Value = text
            .parse()
            .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
        let package = document
            .get("package")
            .unwrap_or_else(|| panic!("{} has no `[package]` table", path.display()))
            .clone();
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("{} has no `package.name`", path.display()))
            .to_owned();

        let mut dependencies = BTreeSet::new();
        for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(entries) = document.get(table).and_then(toml::Value::as_table) else {
                continue;
            };
            for (key, value) in entries {
                // `foo = { package = "bar" }` renames: the *package* is what gets published.
                //
                // **And the rename usually is not here.** A member writes
                // `connector-spec.workspace = true`, so the alias lives in the *root* manifest's
                // `[workspace.dependencies]`, not beside the member's use of it. Reading only the
                // member's table yields the alias and silently drops the edge — which is exactly
                // what happened when the four published crates were renamed to `codewandler-*`:
                // this recomputation lost `codewandler-connector-spec` from the closure and
                // ordered a crate before its own dependency, while every crate still compiled.
                let published = value
                    .get("package")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| workspace_dependency_package(key))
                    .unwrap_or_else(|| key.clone());
                dependencies.insert(published);
            }
        }

        Self {
            name,
            dependencies,
            package,
        }
    }

    /// Whether `[package]` sets this field, directly or by inheriting it from the workspace.
    /// An inherited field is `field.workspace = true`, which is a table rather than a value.
    fn declares(&self, field: &str) -> bool {
        let Some(value) = self.package.get(field) else {
            return false;
        };
        if value
            .get("workspace")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false)
        {
            return true;
        }
        match value {
            toml::Value::String(text) => !text.trim().is_empty(),
            toml::Value::Array(items) => !items.is_empty(),
            _ => false,
        }
    }

    /// The `readme` path this crate declares, if it names a file rather than inheriting one.
    fn readme(&self) -> Option<String> {
        self.package
            .get("readme")?
            .as_str()
            .map(str::to_owned)
            .filter(|path| !path.trim().is_empty())
    }
}

/// The workspace root, from this crate's manifest directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate manifest lives two directories below the workspace root")
        .to_path_buf()
}

/// The package a `[workspace.dependencies]` key resolves to, when the root manifest aliases it.
///
/// `connector-spec = { package = "codewandler-connector-spec", … }` in the root means a member
/// writing `connector-spec.workspace = true` depends on the package `codewandler-connector-spec`.
/// Returns `None` when the key is not aliased, so the caller falls back to the key itself.
fn workspace_dependency_package(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(workspace_root().join("Cargo.toml")).ok()?;
    let document: toml::Value = text.parse().ok()?;
    document
        .get("workspace")?
        .get("dependencies")?
        .get(key)?
        .get("package")
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
}
