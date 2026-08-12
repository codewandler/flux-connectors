//! Publishing is the one irreversible thing this repository can do, so the order it happens in and
//! the metadata it burns are asserted here rather than trusted.
//!
//! Five properties, each of which has a way of being wrong that only shows up at release time:
//!
//! 1. **The closure is the closure.** C-190 names three consumable crates —
//!    `connector-catalog`, `connector-secrets`, `connector-pack` — and `connector-secrets`
//!    re-exports the credential address vocabulary, so whatever crate owns that vocabulary is
//!    published too or `connector-secrets` does not resolve for anyone outside this workspace. The
//!    set is computed from the manifests, not listed, so a new edge grows the closure instead of
//!    silently breaking the next release.
//! 2. **No crate in the closure is the compiler.** The derivation above is faithful and has no
//!    opinion: it grew the closure to include `connector-spec` — the IR, both front-ends, the
//!    validator and the lockfile writer — because one re-export reached into it, and a release
//!    would have put all of that on crates.io permanently. That is what C-407 extracted
//!    `connector-address` to end, and [`no_machinery_crate_is_published`] is what stops the edge
//!    growing back.
//! 3. **The order is a topological sort.** crates.io refuses a crate whose dependencies are not yet
//!    live, and a half-published closure cannot be rolled back. `scripts/publish-crates-io.sh`
//!    derives its order from `cargo metadata`; this test recomputes it independently from the
//!    manifests and requires the two to agree.
//! 4. **The metadata is complete.** `description`, `license`, `repository`, `readme` and `keywords`
//!    are what a crates.io page is made of, and none of them can be corrected in a version that has
//!    already been published — only in the next one.
//! 5. **Public package names reach every consumer-facing pointer.** A packaged README must tell
//!    consumers to depend on the package that actually exists on crates.io, and `documentation`
//!    must point docs.rs at that same published package rather than at the local crate name.
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

/// **The machinery: the crates that turn a provider definition into a generated artifact.** None of
/// them may be published — see [`no_machinery_crate_is_published`].
///
/// This is not `dependency_fence.rs`'s `COMPILER_CRATES`, and the two must not be merged. That list
/// answers *may this crate reach a socket?*, which is why `connector-catalog` is in it even though
/// it is published on purpose. This one answers *is this crate the compiler?*
const MACHINERY: &[&str] = &[
    "codewandler-connector-spec",
    "connector-flux",
    "connector-cli",
];

/// Everything else this workspace builds: the address vocabulary, the generated data, the host
/// libraries and the reference host.
///
/// Being named here is **not** permission to publish — `connectors-api` sets `publish = false`, and
/// what actually reaches crates.io is [`ROOTS`] and what it reaches. It is only the statement that
/// this crate is not part of the compiler, which is the half of the classification
/// [`every_member_is_classified_as_machinery_or_not`] needs in order to be total.
const NOT_MACHINERY: &[&str] = &[
    "codewandler-connector-address",
    "codewandler-connector-catalog",
    // The catalog-pack reader (C-537): generated data plus the dependency-free code that serves
    // it, consumable on its own and published behind `connector-catalog`, which re-exports it. It
    // is data, not the compiler — the crate that *writes* the pack is `connector-cli`, which stays
    // in `MACHINERY` above.
    "codewandler-connector-catalog-reader",
    "codewandler-connector-pack",
    // The plan-deriving core (C-538): the canonical document in, the request plan out. It is
    // published because `connector-pack` depends on it — a consumer of the pack does not resolve
    // without it — and it joins the closure by that derived edge rather than by being a root. It is
    // not the compiler: it reads the artifact the compiler writes and never a provider file.
    "codewandler-connector-resolve",
    "codewandler-connector-secrets",
    "connectors-api",
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

/// Acceptance (C-407): "the published closure contains no compiler crate. Assert the *property*,
/// not the current membership."
///
/// The closure is **derived**, which is the right design and is also why it needs this: a
/// derivation reports what the edges say and cannot object to them. One re-export of
/// `CredentialRef` from `connector-secrets` was enough to put 11,832 lines of compiler — the IR,
/// provider TOML and OpenAPI ingest, validation, the lockfile writer — into a set that, once a
/// `vX.Y.Z` tag is pushed, is on crates.io permanently.
///
/// So the property is stated over the derived set rather than over a list of what it contains
/// today: whatever the closure grows to, none of it may be the compiler. A future edge from a
/// published crate into `connector-spec` fails here rather than at `git push --tags`.
#[test]
fn no_machinery_crate_is_published() {
    let workspace = Workspace::read();
    let closure: BTreeSet<String> = workspace.publish_order(ROOTS).into_iter().collect();

    // A closure that came back empty would satisfy the loop below while proving nothing.
    assert!(
        closure.len() >= ROOTS.len(),
        "the derived publish closure is {closure:?}, which does not even contain {ROOTS:?}; this \
         test would pass on an empty set"
    );

    for name in MACHINERY {
        let dragged_in: Vec<&str> = ROOTS
            .iter()
            .filter(|root| workspace.closure(root).contains(*name))
            .copied()
            .collect();
        assert!(
            !closure.contains(*name),
            "`{name}` is part of this repository's compiler and it is in the publish closure, \
             reached from {dragged_in:?}.\n\
             Publishing is a consequence of pushing a `vX.Y.Z` tag and a published version cannot \
             be withdrawn, so this edge would put a compiler on crates.io permanently. Extract the \
             vocabulary the root actually needs into a crate of its own (C-407 did this for \
             `connector-address`) rather than widening what ships.\n\
             closure: {closure:?}"
        );
    }
}

/// Acceptance (C-523): a consumer outside this workspace — Exchange — can depend on the runtime pack
/// and the origin vocabulary **together**, from the registry, without the compiler coming with them.
///
/// Three claims, because each fails differently. The pack must *reach* the vocabulary, or the shared
/// `HttpsOrigin` is not shared and the runtime is back to a copied grammar. The vocabulary must be
/// **published**, or a consumer that names both gets one that does not resolve. And neither may drag
/// `connector-spec` in, which is the edge C-407 removed and the one a "just depend on the loader"
/// shortcut would recreate — this is the second, narrower statement of
/// [`no_machinery_crate_is_published`], stated over the one root that now holds the origin grammar.
#[test]
fn the_pack_and_the_origin_vocabulary_are_consumable_together_without_the_compiler() {
    const PACK: &str = "codewandler-connector-pack";
    const ADDRESS: &str = "codewandler-connector-address";

    let workspace = Workspace::read();
    let reached = workspace.closure(PACK);
    assert!(
        reached.contains(ADDRESS),
        "`{PACK}` does not depend on `{ADDRESS}`, so the HTTPS-origin grammar it enforces at runtime \
         is a copy rather than the published one: {reached:?}"
    );

    let order = workspace.publish_order(ROOTS);
    let position = |name: &str| {
        order
            .iter()
            .position(|published| published == name)
            .unwrap_or_else(|| panic!("`{name}` is not in the publish order: {order:?}"))
    };
    assert!(
        position(ADDRESS) < position(PACK),
        "`{ADDRESS}` must be live on crates.io before `{PACK}` names it: {order:?}"
    );

    for machinery in MACHINERY {
        assert!(
            !reached.contains(*machinery),
            "`{PACK}` reaches `{machinery}`. Exchange consumes this pack from the registry, so this \
             edge would either publish the compiler or leave the pack unresolvable: {reached:?}"
        );
    }
}

/// [`MACHINERY`] is only a property while it is complete. Without this, a second emitter or a second
/// front-end crate is simply not asked about: it is in neither list, so nothing objects when a
/// published crate grows an edge into it.
///
/// Both directions, because each fails a different way. A member in neither list is a crate whose
/// right to be published nobody decided; a list naming a crate that is not a member is a rename that
/// silently emptied the fence.
#[test]
fn every_member_is_classified_as_machinery_or_not() {
    let workspace = Workspace::read();
    let members: BTreeSet<&str> = workspace.members().map(String::as_str).collect();

    for name in &members {
        let machinery = MACHINERY.contains(name);
        let not = NOT_MACHINERY.contains(name);
        assert!(
            machinery != not,
            "`{name}` is a workspace member named in {} of `MACHINERY` and `NOT_MACHINERY`.\n\
             Name it in exactly one, with a comment saying why. A crate nobody classified is a \
             crate that can join the publish closure without anyone deciding it may.",
            if machinery { "both" } else { "neither" }
        );
    }

    for name in MACHINERY.iter().chain(NOT_MACHINERY) {
        assert!(
            members.contains(name),
            "`{name}` is classified here but is not a member of this workspace: {members:?}.\n\
             A renamed or removed crate leaves a list that still looks like a fence and guards \
             nothing."
        );
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

/// Package names and Rust crate names deliberately differ: consumers install the prefixed package
/// from crates.io, then import the shorter `[lib]` name. Keep the install-facing README and docs.rs
/// metadata on the package side of that boundary.
#[test]
fn published_readmes_and_documentation_use_public_package_names() {
    let workspace = Workspace::read();
    let published = workspace.publish_order(ROOTS);
    let workspace_version = workspace_version();
    let release_line = workspace_release_line(&workspace_version);
    let mut mistakes: Vec<String> = Vec::new();

    for name in &published {
        let manifest = workspace.manifest(name);
        let expected_docs = format!("https://docs.rs/{name}");
        if manifest.string("documentation") != Some(expected_docs.as_str()) {
            mistakes.push(format!(
                "{name}: documentation must be `{expected_docs}`, found {:?}",
                manifest.string("documentation")
            ));
        }

        let Some(readme_path) = manifest.readme() else {
            mistakes.push(format!("{name}: no packaged README to verify"));
            continue;
        };
        let path = workspace.directory(name).join(&readme_path);
        let readme = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let expected_dependency = format!("{name} = \"{release_line}\"");
        if !readme
            .lines()
            .any(|line| line.trim() == expected_dependency)
        {
            mistakes.push(format!(
                "{name}: {readme_path} must contain the uncommented dependency example \
                 `{expected_dependency}` for workspace release {}",
                workspace_version
            ));
        }

        let dependency_prefix = format!("{name} =");
        for line in readme.lines() {
            let install_text = line.trim().trim_start_matches("# ").trim();
            let Some(right_hand_side) = install_text.strip_prefix(&dependency_prefix) else {
                continue;
            };
            let snippet = format!("dependency = {}", right_hand_side.trim());
            let requirement = snippet
                .parse::<toml::Value>()
                .ok()
                .and_then(|document| document.get("dependency").cloned())
                .and_then(|dependency| {
                    dependency.as_str().map(str::to_owned).or_else(|| {
                        dependency
                            .as_table()
                            .and_then(|table| table.get("version"))
                            .and_then(toml::Value::as_str)
                            .map(str::to_owned)
                    })
                });
            if requirement.as_deref() != Some(release_line.as_str()) {
                mistakes.push(format!(
                    "{name}: every dependency example in {readme_path} must require \
                     `{release_line}` for workspace release {workspace_version}; found \
                     {requirement:?} in `{}`",
                    line.trim()
                ));
            }
        }

        for public_name in &published {
            let local_name = public_name.strip_prefix("codewandler-").unwrap_or_else(|| {
                panic!("published package `{public_name}` has no `codewandler-` prefix")
            });
            let stale_dependency = format!("{local_name} =");
            let stale_cargo_add = format!("cargo add {local_name}");
            let stale_crates_io = format!("https://crates.io/crates/{local_name}");
            for line in readme.lines() {
                let install_text = line.trim().trim_start_matches("# ").trim();
                if install_text.starts_with(&stale_dependency)
                    || line.contains(&stale_cargo_add)
                    || line.contains(&stale_crates_io)
                {
                    mistakes.push(format!(
                        "{name}: {readme_path} uses unpublished package name `{local_name}` in `{}`",
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        mistakes.is_empty(),
        "published package pointers do not match the crates.io package names:\n  {}",
        mistakes.join("\n  ")
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

    /// Every member of this workspace, by package name, in a stable order.
    fn members(&self) -> impl Iterator<Item = &String> {
        self.manifests.keys()
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

    /// A direct string value from `[package]`.
    fn string(&self, field: &str) -> Option<&str> {
        self.package.get(field)?.as_str()
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

/// The complete workspace package version, which every published member inherits.
fn workspace_version() -> String {
    let path = workspace_root().join("Cargo.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let document: toml::Value = text
        .parse()
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    document
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("{} has no `[workspace.package].version`", path.display()))
        .to_owned()
}

/// Cargo's recommended dependency requirement for this pre-1.0 release: its major/minor line.
fn workspace_release_line(version: &str) -> String {
    let components: Vec<&str> = version.split('.').collect();
    assert!(
        components.len() == 3
            && components.iter().all(|component| {
                !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
            }),
        "[workspace.package].version `{version}` is not numeric major.minor.patch"
    );
    format!("{}.{}", components[0], components[1])
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
