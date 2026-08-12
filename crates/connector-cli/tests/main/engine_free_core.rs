//! **The plan-deriving core links no engine** (C-538).
//!
//! `connector_pack::resolve` returns `flux_core::Result<Arc<dyn Tool>>`, so a consumer that wants
//! nothing but a connector's *request* — Exchange's settings page, a connection check, a rehearsal —
//! resolves a `codewandler-flux-*` line to get it, and moves when that line moves even though no
//! catalogue content changed (`docs/designs/catalog-artifact.md` §3). `codewandler-connector-resolve`
//! is that derivation with the engine taken out, and this file is what makes the claim structural
//! rather than a paragraph in a manifest.
//!
//! # Why not `Cargo.lock`, when `dependency_fence.rs` reads exactly that
//!
//! Because the lock records **dev**-dependency edges, and one of them makes the answer wrong in the
//! direction that matters: `codewandler-connector-catalog` takes `flux-lang` as a dev-dependency —
//! its `tests/embedded_operations.rs` parses every rendering — so a lock-based walk reports
//! `connector-resolve -> connector-catalog -> flux-lang` and calls the core engine-coupled. No
//! consumer resolves that edge; `cargo` does not build a dependency's dev-dependencies.
//!
//! So this reads the graph **cargo itself resolves, with features applied and kinds filtered** —
//! `cargo metadata --locked --offline`, whose `resolve.nodes[].deps` carry a `dep_kinds` array — and
//! follows only edges a consumer would. It is the same instrument, and the same reasoning, as
//! [`pack_links_no_http_client.rs`](../pack_links_no_http_client.rs), which changed to it for the
//! mirror-image reason: an *optional* dependency the lock records and no build takes.
//!
//! `--locked` and `--offline` are both deliberate: this must describe the committed lockfile, must
//! not be able to change it, and a test in this repository reaching the network would be its own
//! defect.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The crate under fence.
const CORE: &str = "codewandler-connector-resolve";

/// **The engine line, by package prefix.**
///
/// Every crate flux publishes carries it, so the fence is stated over the prefix rather than over a
/// list — a list is wrong the moment flux adds a crate, and wrong *silently*, which is the failure
/// mode a fence must not have.
///
/// `codewandler-flux-spec` is deliberately inside the prefix even though it moves on its own `1.x`
/// line: it is the frozen wire vocabulary a guest plugin compiles against, and a plan that named a
/// `flux_spec::ToolSpec` would still be a plan a consumer needs flux's vocabulary to read. The whole
/// point of the split is that the plan is **data**.
const ENGINE_PREFIX: &str = "codewandler-flux-";

/// Acceptance: "it carries no `codewandler-flux-*` dependency, and a dependency-direction test pins
/// that".
#[test]
fn the_plan_deriving_core_links_no_engine_crate() {
    let graph = Resolved::consumer_graph();

    assert!(
        graph.contains(CORE),
        "`{CORE}` is not a package in {}; this fence has nothing to fence",
        graph.source
    );

    let reached: Vec<&String> = graph
        .closure(CORE)
        .into_iter()
        .filter(|name| name.starts_with(ENGINE_PREFIX))
        .collect::<Vec<_>>()
        .leak()
        .iter()
        .collect();

    assert!(
        reached.is_empty(),
        "`{CORE}` reaches the engine: {reached:?}\n\
         Its acceptance (C-538) is that a consumer who wants a connector's request plan does not \
         resolve a `{ENGINE_PREFIX}*` line to get it. The edge to break is: {}",
        reached
            .first()
            .and_then(|target| graph.path_to(CORE, target))
            .unwrap_or_else(|| "<no path found>".to_owned())
    );
}

/// **The control.** A fence that finds no engine has said one of two things, and they are not the
/// same: *the core is engine-free*, or *this walk cannot see the engine at all*.
///
/// So state that the engine really is in this workspace's consumer graph, reachable from the crate
/// that is on it by design.
#[test]
fn the_engine_is_really_in_this_workspaces_consumer_graph() {
    let graph = Resolved::consumer_graph();
    let pack = graph.closure("codewandler-connector-pack");
    let engine: Vec<&String> = pack
        .iter()
        .filter(|name| name.starts_with(ENGINE_PREFIX))
        .collect();

    assert!(
        !engine.is_empty(),
        "no `{ENGINE_PREFIX}*` crate is reachable from `codewandler-connector-pack` in {}, so the \
         fence above is asserting the absence of something that is absent everywhere",
        graph.source
    );
}

/// **And the walk itself**, over a graph stated directly — including the dev-edge shape that made
/// the lockfile the wrong instrument.
#[test]
fn the_walk_follows_a_transitive_edge_and_stops_at_a_severed_one() {
    let coupled = Resolved::over(&[
        (CORE, &["codewandler-connector-catalog"]),
        ("codewandler-connector-catalog", &["codewandler-flux-lang"]),
        ("codewandler-flux-lang", &[]),
    ]);
    assert!(coupled
        .closure(CORE)
        .iter()
        .any(|name| name.starts_with(ENGINE_PREFIX)));
    assert_eq!(
        coupled.path_to(CORE, "codewandler-flux-lang").as_deref(),
        Some("codewandler-connector-resolve -> codewandler-connector-catalog -> codewandler-flux-lang")
    );

    // The real shape: the catalog's `flux-lang` edge is a dev-dependency, which a consumer graph
    // does not carry, so the same walk reports nothing.
    let severed = Resolved::over(&[
        (CORE, &["codewandler-connector-catalog"]),
        (
            "codewandler-connector-catalog",
            &["codewandler-connector-catalog-reader"],
        ),
        ("codewandler-connector-catalog-reader", &[]),
    ]);
    assert!(!severed
        .closure(CORE)
        .iter()
        .any(|name| name.starts_with(ENGINE_PREFIX)));
    assert_eq!(severed.path_to(CORE, "codewandler-flux-lang"), None);
}

/// The dependency graph cargo resolved, with features applied and only the edges a **consumer**
/// links.
struct Resolved {
    /// Where it came from, for a failure message.
    source: String,
    /// Package name to the names of its normal dependencies.
    packages: BTreeMap<String, Vec<String>>,
}

impl Resolved {
    /// Ask cargo for the resolved graph, keeping `[dependencies]` edges only.
    ///
    /// Package *names* collapse versions, which is the same simplification `dependency_fence.rs`
    /// makes: for a fence keyed on "is this crate here at all" the version is not the question.
    ///
    /// # Panics
    ///
    /// If cargo cannot be run, exits non-zero, or emits something that is not the metadata
    /// document — in a test each of those is the assertion failing, not a condition to recover from.
    fn consumer_graph() -> Self {
        let root = workspace_root();
        let output = Command::new(env!("CARGO"))
            .args(["metadata", "--format-version", "1", "--locked", "--offline"])
            .current_dir(&root)
            .output()
            .unwrap_or_else(|error| panic!("run `cargo metadata` in {}: {error}", root.display()));
        assert!(
            output.status.success(),
            "`cargo metadata` failed in {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr)
        );

        let document: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("`cargo metadata` emits JSON");

        // Package ids are opaque; the names live on the package entries.
        let mut name_of: BTreeMap<&str, &str> = BTreeMap::new();
        for package in document["packages"]
            .as_array()
            .expect("`packages` is an array")
        {
            let (Some(id), Some(name)) = (package["id"].as_str(), package["name"].as_str()) else {
                continue;
            };
            name_of.insert(id, name);
        }

        let mut packages: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for node in document["resolve"]["nodes"]
            .as_array()
            .expect("`resolve.nodes` is an array — `--no-deps` was not passed")
        {
            let Some(name) = node["id"].as_str().and_then(|id| name_of.get(id)) else {
                continue;
            };
            let mut dependencies = Vec::new();
            for dep in node["deps"].as_array().into_iter().flatten() {
                // `cargo metadata` spells a normal dependency as a null `kind`; `dev` and `build`
                // name themselves. No `dep_kinds` at all is an older metadata format, treated as
                // normal rather than silently dropping the edge.
                let normal = dep["dep_kinds"]
                    .as_array()
                    .map(|entries| entries.iter().any(|entry| entry["kind"].is_null()))
                    .unwrap_or(true);
                if !normal {
                    continue;
                }
                if let Some(dependency) = dep["pkg"].as_str().and_then(|id| name_of.get(id)) {
                    dependencies.push((*dependency).to_owned());
                }
            }
            packages.insert((*name).to_owned(), dependencies);
        }

        Self {
            source: format!("the feature-resolved consumer graph of {}", root.display()),
            packages,
        }
    }

    /// A graph stated directly, for asserting the walk itself.
    fn over(edges: &[(&str, &[&str])]) -> Self {
        Self {
            source: "<synthetic>".to_owned(),
            packages: edges
                .iter()
                .map(|(name, dependencies)| {
                    (
                        (*name).to_owned(),
                        dependencies.iter().map(|d| (*d).to_owned()).collect(),
                    )
                })
                .collect(),
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.packages.contains_key(name)
    }

    /// Every package reachable from `root`, `root` excluded.
    fn closure(&self, root: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut pending = vec![root.to_owned()];
        while let Some(name) = pending.pop() {
            for dependency in self.packages.get(&name).into_iter().flatten() {
                if seen.insert(dependency.clone()) {
                    pending.push(dependency.clone());
                }
            }
        }
        seen
    }

    /// A rendered `a -> b -> c` chain from `root` to `target`, breadth first, so the reported chain
    /// is a shortest one and names the edge worth deleting.
    fn path_to(&self, root: &str, target: &str) -> Option<String> {
        let mut previous: BTreeMap<String, String> = BTreeMap::new();
        let mut queue = std::collections::VecDeque::from([root.to_owned()]);
        let mut seen = BTreeSet::from([root.to_owned()]);
        while let Some(name) = queue.pop_front() {
            for dependency in self.packages.get(&name).into_iter().flatten() {
                if !seen.insert(dependency.clone()) {
                    continue;
                }
                previous.insert(dependency.clone(), name.clone());
                if dependency == target {
                    let mut chain = vec![target.to_owned()];
                    let mut step = target;
                    while let Some(parent) = previous.get(step) {
                        chain.push(parent.clone());
                        step = parent;
                    }
                    chain.reverse();
                    return Some(chain.join(" -> "));
                }
                queue.push_back(dependency.clone());
            }
        }
        None
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
