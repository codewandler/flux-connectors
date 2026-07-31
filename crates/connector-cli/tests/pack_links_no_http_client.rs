//! `connector-pack` holds no HTTP client, and this is where that stops being a comment.
//!
//! The claim is stated twice in the source. `crates/connector-pack/src/tool.rs` gives it as the
//! reason [`Egress`] is a newtype over `Arc<dyn Tool>` rather than a concrete
//! `flux_web::http::HttpRequestTool`: *"It keeps this crate from linking `flux-web` — a whole HTTP
//! client, a DNS resolver and an SSRF guard — into a library whose entire claim is that it opens no
//! socket, so the claim stays structural rather than merely true today."* `AGENTS.md`'s ownership
//! table says the same in `connector-pack`'s "Must never" column. Until C-199 neither sentence was
//! tested, and `connector-pack` appears in neither list in
//! [`dependency_fence.rs`](../dependency_fence.rs), so adding `flux-web` to it tripped nothing.
//!
//! # Which graph this reads, and why it is not the lockfile
//!
//! [`dependency_fence.rs`](../dependency_fence.rs) reads `Cargo.lock`, and that choice is
//! load-bearing there: the lock records the resolved graph **including optional dependencies**, so
//! an edge added behind a feature flag trips it too. **That instrument cannot state this
//! guarantee.** `connector-pack` depends on `connector-secrets` — legitimately; it is where a
//! credential is resolved — and `connector-secrets` declares an optional `reqwest` behind its
//! `vault` feature. So the lock walk already reports a path, measured against the committed lock:
//!
//! ```text
//! codewandler-connector-pack -> codewandler-connector-secrets -> reqwest
//! codewandler-connector-pack -> codewandler-connector-secrets -> reqwest -> hyper
//! ```
//!
//! Both chains describe a build that does not happen. `vault` is off by default, so no `reqwest` is
//! compiled and none is linked. A fence written in the lockfile idiom would therefore be red on a
//! correct tree, and the only honest options were to change instrument or to special-case the edge.
//!
//! **This file changes instrument: it reads the graph cargo itself resolves, with features
//! applied** — `cargo metadata --locked --offline`, whose `resolve.nodes[].deps` omit an optional
//! dependency whose feature is off. It is the same graph `cargo tree -e normal` prints, taken as
//! JSON so the assertion does not depend on a human-readable layout.
//!
//! # What that consequently cannot see, and what covers each gap
//!
//! Giving up the lock means giving up its optional-dependency coverage, so each thing the new
//! instrument is blind to is named and covered rather than left implied:
//!
//! 1. **An optional edge a *host* switches on.** Cargo unifies features, so a host that puts
//!    `codewandler-connector-secrets` in its own manifest with `features = ["vault"]` gets one
//!    `connector_secrets` rlib with `reqwest` in it, and the pack links that. The pack cannot
//!    prevent it and this graph cannot see it, because the host is not in this workspace.
//!    [`the_optional_http_client_behind_the_pack_is_off_by_default_and_unrequested`] states the
//!    reason it does not happen by accident — the carrier is `optional`, `default = []`, and no
//!    manifest here asks for the feature — over the manifests, which is where that fact lives.
//! 2. **A feature set selected on the pack itself.** This graph is the default one, and "default"
//!    only covers every build a host can ask for if there is nothing else to ask for.
//!    [`the_pack_declares_no_features_so_the_default_graph_is_every_host_selectable_build`] holds
//!    that: the pack declares no `[features]` table, so its default feature set is its only one.
//! 3. **A client whose name is not in [`HTTP_CLIENTS`].** This is a denylist, and a denylist is
//!    evadable by construction. It is the acknowledged weak edge of this file; the mitigation is
//!    that the list names the clients that actually resolve in this workspace, and
//!    [`the_denylist_names_a_client_this_workspace_really_resolves`] fails if it ever stops doing
//!    so — an all-absent denylist would pass forever while asserting nothing.
//! 4. **Dev- and build-dependencies**, which a consumer never links. The primary fence excludes
//!    them deliberately, because "links" is the claim. They are asserted separately and more weakly
//!    by [`the_packs_own_test_build_links_no_http_client_either`], because `AGENTS.md` records that
//!    the pack's own tests pass a stub — a dev-dependency on a real client is the drift that would
//!    make the stub optional.
//!
//! One thing this reads *more* of than the claim strictly needs: no `--filter-platform` is passed,
//! so the graph is the union over every target. For a fence that is the safe direction — it is a
//! superset of what any one platform links.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The crate whose claim this file makes structural.
const PACK: &str = "codewandler-connector-pack";

/// **The HTTP clients, named so the fence is a decision rather than a silence.**
///
/// The first three are in this workspace's resolved graph today, through `connectors-api`, which is
/// the one crate here allowed to hold one. The rest are the alternatives someone reaching for a
/// client would plausibly reach for. See gap 3 in the module comment for what a denylist cannot do.
const HTTP_CLIENTS: &[&str] = &[
    "codewandler-flux-web",
    "reqwest",
    "hyper",
    "ureq",
    "isahc",
    "curl",
];

/// The dependency that *can* carry an HTTP client into the pack, and the feature that would do it.
const CARRIER: &str = "codewandler-connector-secrets";
/// The key `[workspace.dependencies]` gives [`CARRIER`], which is what member manifests write.
const CARRIER_KEY: &str = "connector-secrets";
/// The feature on [`CARRIER`] that turns `dep:reqwest` on.
const CARRIER_FEATURE: &str = "vault";

/// **The fence.** Acceptance: "A test asserts `codewandler-connector-pack` links no HTTP client —
/// `codewandler-flux-web`, `reqwest`, `hyper`, `ureq`, `isahc`, `curl` — under the feature sets a
/// host can select."
#[test]
fn connector_pack_links_no_http_client() {
    let graph = Resolved::read(Kinds::Normal);

    // A fence around a crate that is not in the graph is vacuous, and would pass for exactly as
    // long as it took someone to notice.
    assert!(
        graph.contains(PACK),
        "`{PACK}` is not in {}; this fence has nothing to fence",
        graph.source
    );

    for client in HTTP_CLIENTS {
        assert!(
            !graph.closure(PACK).contains(*client),
            "`{PACK}` links `{client}`: {}\n\
             The pack's `Egress` is `Arc<dyn Tool>` precisely so this edge does not exist — a host \
             supplies the client it has already configured with its egress allow-list and its SSRF \
             guard. Take the concrete client back out and pass it to `Egress::new` instead.",
            graph
                .path_to(PACK, client)
                .unwrap_or_else(|| "<no path found>".to_owned()),
        );
    }
}

/// The same question asked of the build the pack's *own* tests run in.
///
/// A consumer never links a dev-dependency, so this is a weaker claim than the one above and is
/// kept separate rather than folded in. It is worth asserting because `AGENTS.md` records the
/// discipline it protects: *"`connector-pack`'s own tests still pass a stub, and still say so — the
/// crate must never link a client."* The tempting shortcut is a dev-dependency on `flux-web` to
/// "test against the real thing", which retires the stub without touching a single line of shipped
/// code.
#[test]
fn the_packs_own_test_build_links_no_http_client_either() {
    let graph = Resolved::read(Kinds::All);
    assert!(graph.contains(PACK), "`{PACK}` is not in {}", graph.source);

    for client in HTTP_CLIENTS {
        assert!(
            !graph.closure(PACK).contains(*client),
            "`{PACK}`'s test build links `{client}`: {}\n\
             If `connector_pack_links_no_http_client` is green, this is a dev- or build-dependency \
             and shipped code is still clean. The pack's tests drive `Egress` with a stub transport \
             on purpose; a real client here retires that stub silently.",
            graph
                .path_to(PACK, client)
                .unwrap_or_else(|| "<no path found>".to_owned()),
        );
    }
}

/// **The `connector-secrets` edge, head-on.** Acceptance: "the test states why a dependency that
/// *can* carry an optional HTTP client does not put one in the pack."
///
/// Three facts make the chain the lockfile reports a chain no build takes, and each is asserted
/// where it lives — in a manifest — rather than inferred from the resolved graph, which is the
/// thing that would silently agree with a mistake:
///
/// 1. the client is `optional`, so it is a `dep:` behind a feature and not an edge;
/// 2. the carrier's `default` feature set does not contain that feature;
/// 3. nothing in this workspace asks for the feature — not as a dependency feature, and not
///    through a feature of its own forwarding `connector-secrets/vault`.
///
/// The third is quantified over `[workspace] members`, not over a list kept here, so a new crate
/// that switches `vault` on is covered by existing. Feature unification is why it must be the whole
/// workspace and not just the pack: one member turning `vault` on puts `reqwest` in the single
/// `connector_secrets` rlib that every member links, the pack included.
#[test]
fn the_optional_http_client_behind_the_pack_is_off_by_default_and_unrequested() {
    let root = workspace_root();
    let carrier = manifest_of(&root.join("crates/connector-secrets/Cargo.toml"));

    // 1. The carrier is optional. Without this the other two say nothing.
    let optional = carrier
        .get("dependencies")
        .and_then(|table| table.get("reqwest"))
        .and_then(|entry| entry.get("optional"))
        .and_then(toml::Value::as_bool);
    assert_eq!(
        optional,
        Some(true),
        "`{CARRIER}` no longer declares `reqwest` as `optional = true`. The chain \
         `{PACK} -> {CARRIER} -> reqwest` that `Cargo.lock` reports would then be a real edge, and \
         this file's whole argument for reading a feature-resolved graph instead is void."
    );

    // 2. It is off by default.
    let features = carrier
        .get("features")
        .unwrap_or_else(|| panic!("`{CARRIER}` declares no `[features]`, so `{CARRIER_FEATURE}` is not a feature and this test is asserting about nothing"));
    assert!(
        features.get(CARRIER_FEATURE).is_some(),
        "`{CARRIER}` no longer declares a `{CARRIER_FEATURE}` feature; re-derive what carries \
         `reqwest` before trusting this file"
    );
    let default: Vec<&str> = features
        .get("default")
        .and_then(toml::Value::as_array)
        .map(|values| values.iter().filter_map(toml::Value::as_str).collect())
        .unwrap_or_default();
    assert!(
        !default.contains(&CARRIER_FEATURE),
        "`{CARRIER}`'s `default` feature set is {default:?}, which switches `{CARRIER_FEATURE}` on. \
         Every crate depending on `{CARRIER}` — `{PACK}` among them — then links `reqwest` without \
         naming it."
    );

    // 3. Nobody here asks for it.
    let mut checked = 0usize;
    for (member, manifest) in workspace_members(&root) {
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            for key in [CARRIER_KEY, CARRIER] {
                let requested: Vec<&str> = manifest
                    .get(section)
                    .and_then(|table| table.get(key))
                    .and_then(|entry| entry.get("features"))
                    .and_then(toml::Value::as_array)
                    .map(|values| values.iter().filter_map(toml::Value::as_str).collect())
                    .unwrap_or_default();
                assert!(
                    !requested.contains(&CARRIER_FEATURE),
                    "`{member}` requests `{key}/{CARRIER_FEATURE}` in `[{section}]`. Cargo unifies \
                     features across the workspace, so this puts `reqwest` in the one \
                     `connector_secrets` rlib that `{PACK}` links too."
                );
            }
        }

        // The same request written the long way round: a feature of one's own that forwards.
        let forwarded = format!("{CARRIER_KEY}/{CARRIER_FEATURE}");
        let forwarded_alias = format!("{CARRIER}/{CARRIER_FEATURE}");
        if let Some(table) = manifest.get("features").and_then(toml::Value::as_table) {
            for (name, values) in table {
                let entries: Vec<&str> = values
                    .as_array()
                    .map(|values| values.iter().filter_map(toml::Value::as_str).collect())
                    .unwrap_or_default();
                assert!(
                    !entries.contains(&forwarded.as_str())
                        && !entries.contains(&forwarded_alias.as_str()),
                    "`{member}`'s `{name}` feature forwards to `{forwarded}`, which switches the \
                     Vault client on for everything that links `{CARRIER}`, `{PACK}` included."
                );
            }
        }
        checked += 1;
    }
    assert!(
        checked > 0,
        "no workspace members were read, so this asserted nothing"
    );
}

/// **What makes "under the default features" mean "under every build a host can ask for".**
///
/// [`connector_pack_links_no_http_client`] reads one graph, the default one. That is only the
/// acceptance's "feature sets a host can select" if the pack offers a host nothing to select — and
/// it does not: it declares no `[features]` table at all, so its feature set is empty and its
/// default is its only one. The day it grows one, this fails and says which graphs the fence above
/// has stopped covering.
#[test]
fn the_pack_declares_no_features_so_the_default_graph_is_every_host_selectable_build() {
    let manifest = manifest_of(&workspace_root().join("crates/connector-pack/Cargo.toml"));
    let declared: Vec<&String> = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().collect())
        .unwrap_or_default();

    assert!(
        declared.is_empty(),
        "`{PACK}` now declares features {declared:?}. `connector_pack_links_no_http_client` reads \
         the default graph only, so it no longer covers every build a host can select. Either \
         assert the additional feature sets there, or record here why they cannot reach a client."
    );
}

/// **A denylist of names that appear nowhere passes forever while asserting nothing.**
///
/// This is the non-vacuity check for [`HTTP_CLIENTS`] itself, and it is stronger than a spelling
/// test: it asserts the workspace really does resolve a named HTTP client somewhere, so the fence
/// above is separating the pack from a client that is *in this very graph* rather than from a
/// hypothetical one. Today `connectors-api` supplies it, which is exactly the arrangement
/// `dependency_fence.rs`'s `NETWORK_CRATES` records.
#[test]
fn the_denylist_names_a_client_this_workspace_really_resolves() {
    let graph = Resolved::read(Kinds::All);
    let present: Vec<&&str> = HTTP_CLIENTS
        .iter()
        .filter(|client| graph.contains(client))
        .collect();

    assert!(
        !present.is_empty(),
        "none of {HTTP_CLIENTS:?} resolves anywhere in {}. Either the workspace stopped shipping a \
         host, or these names have gone stale — and a denylist of names nothing matches is a test \
         that cannot fail.",
        graph.source
    );
}

/// The fence is only as good as the walk under it, and the walk is the part that could pass while
/// seeing nothing. Written against a synthetic graph for the same reason
/// `dependency_fence.rs::the_walk_finds_an_edge_that_is_not_direct` is: the case that would actually
/// happen is **transitive** — some crate the pack already depends on takes a client — and proving
/// that against the real graph would mean editing another crate's manifest.
///
/// Acceptance: "The fence is non-vacuous: it is proved to catch a real edge, either against a
/// synthetic graph … or by a recorded manual run with the edge temporarily added." Both were done;
/// this is the half that stays in the repository.
#[test]
fn the_walk_finds_a_client_that_is_not_direct() {
    let graph = Resolved::over(&[
        (PACK, &[CARRIER, "codewandler-flux-runtime", "serde_json"]),
        (CARRIER, &["codewandler-connector-spec"]),
        ("codewandler-flux-runtime", &["tokio", "helper"]),
        ("helper", &["reqwest"]),
        ("reqwest", &["hyper"]),
        ("hyper", &[]),
        ("tokio", &[]),
        ("serde_json", &[]),
        ("codewandler-connector-spec", &[]),
    ]);

    let closure = graph.closure(PACK);
    assert!(
        closure.contains("reqwest"),
        "a client three edges away must still be in the closure: {closure:?}"
    );
    // And what the operator is told is the chain to break, not merely that one exists.
    assert_eq!(
        graph.path_to(PACK, "reqwest").as_deref(),
        Some(format!("{PACK} -> codewandler-flux-runtime -> helper -> reqwest").as_str())
    );
    assert_eq!(
        graph.path_to(PACK, "hyper").as_deref(),
        Some(format!("{PACK} -> codewandler-flux-runtime -> helper -> reqwest -> hyper").as_str())
    );

    // The control: with the one offending edge removed, the same walk reports nothing. Without
    // this, a walk that reported every pair would satisfy the assertions above.
    let severed = Resolved::over(&[
        (PACK, &[CARRIER, "codewandler-flux-runtime", "serde_json"]),
        (CARRIER, &["codewandler-connector-spec"]),
        ("codewandler-flux-runtime", &["tokio", "helper"]),
        ("helper", &["tokio"]),
        ("reqwest", &["hyper"]),
        ("hyper", &[]),
        ("tokio", &[]),
        ("serde_json", &[]),
        ("codewandler-connector-spec", &[]),
    ]);
    assert!(!severed.closure(PACK).contains("reqwest"));
    assert_eq!(severed.path_to(PACK, "reqwest"), None);
}

/// Which dependency kinds a walk follows.
#[derive(Clone, Copy)]
enum Kinds {
    /// `[dependencies]` only — what a consumer of the published crate links.
    Normal,
    /// Every kind, including dev and build — what this workspace's own `cargo test` compiles.
    All,
}

impl Kinds {
    /// `cargo metadata` spells a normal dependency as a null `kind`.
    fn admits(self, kind: Option<&str>) -> bool {
        match self {
            Self::Normal => kind.is_none(),
            Self::All => true,
        }
    }
}

/// The dependency graph cargo resolved, **with features applied**.
struct Resolved {
    /// Where it came from, for a failure message.
    source: String,
    /// Package name to the names of its dependencies of the selected kinds.
    packages: BTreeMap<String, Vec<String>>,
}

impl Resolved {
    /// Ask cargo for the resolved graph and index it by package name.
    ///
    /// `--locked` and `--offline` are both deliberate: this must describe the committed lockfile
    /// and must not be able to change it, and a test in this repository reaching the network would
    /// be its own defect.
    ///
    /// Package *names* collapse versions, which is the same simplification `dependency_fence.rs`
    /// makes. For a fence keyed on "is this crate here at all" the version is not the question.
    ///
    /// # Panics
    ///
    /// If cargo cannot be run, exits non-zero, or emits something that is not the metadata
    /// document — in a test each of those is the assertion failing, not a condition to recover
    /// from.
    fn read(kinds: Kinds) -> Self {
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
                let admitted = dep["dep_kinds"]
                    .as_array()
                    .map(|entries| {
                        entries
                            .iter()
                            .any(|entry| kinds.admits(entry["kind"].as_str()))
                    })
                    // No `dep_kinds` at all is an older metadata format; treat it as normal rather
                    // than silently dropping the edge.
                    .unwrap_or(true);
                if !admitted {
                    continue;
                }
                if let Some(dependency) = dep["pkg"].as_str().and_then(|id| name_of.get(id)) {
                    dependencies.push((*dependency).to_owned());
                }
            }
            packages.insert((*name).to_owned(), dependencies);
        }

        Self {
            source: format!("the feature-resolved graph of {}", root.display()),
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

    /// Whether the graph records a package by this name at all.
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

    /// A rendered `a -> b -> c` chain from `root` to `target`, for the failure message. Breadth
    /// first, so the reported chain is a shortest one and names the edge worth deleting.
    fn path_to(&self, root: &str, target: &str) -> Option<String> {
        let mut previous: BTreeMap<String, String> = BTreeMap::new();
        let mut queue = VecDeque::from([root.to_owned()]);
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

/// Parse one manifest.
fn manifest_of(path: &Path) -> toml::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    text.parse()
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

/// Every workspace member, as `(member path, parsed manifest)`.
///
/// Derived from `[workspace] members` rather than from a list kept here, so a crate added tomorrow
/// is covered without anyone remembering to add it — the same reason
/// `dependency_fence.rs::every_workspace_member_is_classified` reads the manifest.
fn workspace_members(root: &Path) -> Vec<(String, toml::Value)> {
    let workspace = manifest_of(&root.join("Cargo.toml"));
    workspace["workspace"]["members"]
        .as_array()
        .expect("`[workspace] members`")
        .iter()
        .filter_map(toml::Value::as_str)
        .map(|member| {
            (
                member.to_owned(),
                manifest_of(&root.join(member).join("Cargo.toml")),
            )
        })
        .collect()
}

/// The workspace root, from this crate's manifest directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate manifest lives two directories below the workspace root")
        .to_path_buf()
}
