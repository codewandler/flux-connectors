//! `connector-secrets` is a host library, and it must stay outside the compile path.
//!
//! [`no_network.rs`](../no_network.rs) proves that *this* crate never opens a socket during a
//! build. That proof is only as strong as the crate's dependency closure: the moment
//! `connector-secrets` — which ships an HTTP client for Vault — appears anywhere in it, "generation
//! is explicit, committed, deterministic and offline" stops being a statement about the build, and
//! `no_network.rs`'s source audit stops being able to see the violation, because the socket now
//! lives in a different crate's `src/`.
//!
//! So the fence is asserted over the **dependency graph**, not by convention. It is read out of
//! `Cargo.lock`, which is deliberate: the lock records the resolved graph including *optional*
//! dependencies, so adding the edge behind a feature flag trips this too.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The host library. Nothing in the compiler may reach it.
const HOST_LIBRARY: &str = "codewandler-connector-secrets";

/// The crates that make up the offline compiler. `connector-catalog` is in the list because it is
/// the leaf the pipeline writes into and is documented as dependency-free; `connector-address` is
/// in it because it is the address vocabulary the compiler is written against, and a socket
/// reachable from a crate every compiler crate links would end the offline guarantee from below.
///
/// **Being here is about sockets, not about publishing.** Two of these are published on purpose —
/// `connector-catalog` is the crate a consumer adds, and `connector-address` is what
/// `connector-secrets` re-exports — so this list is not the answer to "may this crate reach
/// crates.io?". That question is `publish_closure.rs`'s `MACHINERY`, which deliberately holds a
/// different set (C-407).
const COMPILER_CRATES: &[&str] = &[
    "connector-cli",
    "codewandler-connector-spec",
    "connector-flux",
    "codewandler-connector-catalog",
    "codewandler-connector-address",
];

/// **Crates allowed to open sockets, named so the allowance is a decision rather than a silence.**
///
/// Before C-202 this fence's guarantee was *"the compiler is offline"* — true, valuable, and scoped
/// narrowly enough that a network-capable crate elsewhere in the workspace would have passed it
/// without being looked at. `docs/designs/connectors-app.md` §"The extension: allow the exception,
/// visibly" makes the argument: adding such a crate does not weaken the guarantee, it makes the
/// fence's *silence* ambiguous, because a reader who finds `reqwest` in `Cargo.lock` and a passing
/// test cannot tell whether the edge was considered or merely unexamined.
///
/// `connectors-api` is the host (C-200). It binds ports, holds an HTTP client through `flux-web`,
/// and runs a loop — everything the compiler crates must not do. It is bounded by being a **leaf**:
/// nothing may depend on it, which [`a_compiler_crate_cannot_reach_a_network_crate`] enforces in the
/// direction that matters.
const NETWORK_CRATES: &[&str] = &["connectors-api"];

/// Acceptance: "`connector-cli` does not depend on it — asserted by a test over the dependency
/// graph, not by convention."
#[test]
fn connector_cli_does_not_depend_on_connector_secrets() {
    let lock = Lock::read();

    // A fence around a crate that does not exist is vacuous, and would go on passing for exactly as
    // long as it took someone to notice. Assert the subject is real first.
    assert!(
        lock.contains(HOST_LIBRARY),
        "`{HOST_LIBRARY}` is not a package in {}; this fence has nothing to fence",
        lock.path.display()
    );

    for crate_name in COMPILER_CRATES {
        let closure = lock.closure(crate_name);
        assert!(
            !closure.contains(HOST_LIBRARY),
            "`{crate_name}` depends on `{HOST_LIBRARY}`, directly or transitively: {}\n\
             `{HOST_LIBRARY}` opens sockets, so this edge would put an HTTP client in the compile \
             path and make `no_network.rs` a statement about less than the build.",
            lock.path_to(crate_name, HOST_LIBRARY)
                .unwrap_or_else(|| "<no path found>".to_owned()),
        );
    }
}

/// The host libraries: built and tested here, excluded from the compile path, and opening no socket
/// of their own. They are neither compiler nor network crates, and they are listed so that
/// [`every_workspace_member_is_classified`] has three buckets to sort into rather than an
/// unexplained remainder.
///
/// **Being in this list is a classification, not a fence.** `codewandler-connector-pack`'s own
/// claim — that it links no HTTP client — cannot be stated over `Cargo.lock` at all, because the
/// lock reports the optional `connector-secrets -> reqwest` edge that no build takes. It is
/// asserted over cargo's feature-resolved graph instead, in
/// [`pack_links_no_http_client.rs`](../pack_links_no_http_client.rs) (C-199).
const HOST_LIBRARIES: &[&str] = &["codewandler-connector-pack", HOST_LIBRARY];

/// **The edge that actually earns the allow-list its keep.**
///
/// `NETWORK_CRATES` on its own is a comment. This is what makes it structural: a compiler crate
/// reaching the host — `connector-cli -> connectors-api`, say, to "just reuse the server's types" —
/// puts an HTTP client, a DNS resolver and a listener back in the compile path by a different route
/// than the one [`connector_cli_does_not_depend_on_connector_secrets`] guards.
#[test]
fn a_compiler_crate_cannot_reach_a_network_crate() {
    let lock = Lock::read();

    for network_crate in NETWORK_CRATES {
        assert!(
            lock.contains(network_crate),
            "`{network_crate}` is not a package in {}; this fence has nothing to fence",
            lock.path.display()
        );
        for crate_name in COMPILER_CRATES {
            let closure = lock.closure(crate_name);
            assert!(
                !closure.contains(*network_crate),
                "`{crate_name}` depends on `{network_crate}`, directly or transitively: {}\n\
                 `{network_crate}` is allowed to open sockets precisely because it is a leaf. An \
                 edge into it from the compiler ends the offline guarantee `no_network.rs` states.",
                lock.path_to(crate_name, network_crate)
                    .unwrap_or_else(|| "<no path found>".to_owned()),
            );
        }
    }
}

/// **Every workspace member is deliberately one of three things.**
///
/// Without this, a new crate is simply not asked about: it is not a compiler crate so nothing fences
/// it, and it is not on the allow-list so nothing records that it may open a socket. That is the
/// "merely unexamined" state the allow-list exists to end, and it is why this asserts over the
/// workspace's own membership rather than over a second hand-kept list.
#[test]
fn every_workspace_member_is_classified() {
    let root = workspace_root();
    let manifest =
        std::fs::read_to_string(root.join("Cargo.toml")).expect("the workspace manifest");
    let document: toml::Value = manifest.parse().expect("the workspace manifest parses");
    let members = document
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .expect("`[workspace] members`");

    let mut checked = 0usize;
    for member in members.iter().filter_map(toml::Value::as_str) {
        let path = root.join(member).join("Cargo.toml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let member_document: toml::Value = text
            .parse()
            .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
        let name = member_document
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("{} has no `[package] name`", path.display()))
            .to_owned();

        assert!(
            COMPILER_CRATES.contains(&name.as_str())
                || NETWORK_CRATES.contains(&name.as_str())
                || HOST_LIBRARIES.contains(&name.as_str()),
            "`{name}` ({member}) is a workspace member classified as neither a compiler crate, a \
             host library, nor a network crate.\n\
             Add it to exactly one list in this file, with a comment saying why. A crate nobody \
             classified is a crate whose right to open a socket nobody decided."
        );
        checked += 1;
    }

    assert!(
        checked > 0,
        "no workspace members were read, so this asserted nothing"
    );
}

/// The resolved dependency graph, as `Cargo.lock` records it.
struct Lock {
    path: PathBuf,
    /// Package name to the names of its recorded dependencies.
    packages: BTreeMap<String, Vec<String>>,
}

impl Lock {
    /// Parse the workspace lockfile.
    ///
    /// # Panics
    ///
    /// If the lockfile cannot be found, read or parsed — in a test, each of those is the assertion
    /// failing rather than a condition to recover from.
    fn read() -> Self {
        let path = workspace_root().join("Cargo.lock");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let document: toml::Value = text
            .parse()
            .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));

        let mut packages = BTreeMap::new();
        let entries = document
            .get("package")
            .and_then(toml::Value::as_array)
            .unwrap_or_else(|| panic!("{} has no `[[package]]` entries", path.display()));
        for entry in entries {
            let Some(name) = entry.get("name").and_then(toml::Value::as_str) else {
                continue;
            };
            let dependencies = entry
                .get("dependencies")
                .and_then(toml::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(toml::Value::as_str)
                        // An entry is `name`, `name version`, or `name version (source)`.
                        .filter_map(|value| value.split_whitespace().next())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            packages.insert(name.to_owned(), dependencies);
        }

        Self { path, packages }
    }

    /// A graph stated directly, for asserting the walk itself.
    fn over(edges: &[(&str, &[&str])]) -> Self {
        Self {
            path: PathBuf::from("<synthetic>"),
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

    /// Whether the lockfile records a package by this name at all.
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

/// The fence above is only as good as the walk under it, and the walk is the part that could pass
/// while seeing nothing.
///
/// A **direct** edge was checked by hand: adding `connector-secrets.workspace = true` to
/// `crates/connector-cli/Cargo.toml` fails the test above with
/// `connector-cli -> connector-secrets`. What cannot be checked that way without editing another
/// crate's manifest is the **transitive** case, which is the one that would actually happen — some
/// future crate takes the host library, and `connector-cli` takes that crate. So it is asserted
/// here over a synthetic graph instead.
#[test]
fn the_walk_finds_an_edge_that_is_not_direct() {
    let lock = Lock::over(&[
        ("connector-cli", &["connector-spec", "helper"]),
        ("connector-spec", &["thiserror"]),
        ("helper", &[HOST_LIBRARY]),
        (HOST_LIBRARY, &["reqwest"]),
        ("reqwest", &[]),
        ("thiserror", &[]),
    ]);

    let closure = lock.closure("connector-cli");
    assert!(
        closure.contains(HOST_LIBRARY),
        "a dependency two edges away must still be in the closure: {closure:?}"
    );
    // And what the operator is told is the chain to break, not just that one exists.
    assert_eq!(
        lock.path_to("connector-cli", HOST_LIBRARY).as_deref(),
        Some(format!("connector-cli -> helper -> {HOST_LIBRARY}").as_str())
    );

    // The control: with the middle crate's edge removed, the same walk reports nothing.
    let severed = Lock::over(&[
        ("connector-cli", &["connector-spec", "helper"]),
        ("connector-spec", &["thiserror"]),
        ("helper", &["thiserror"]),
        (HOST_LIBRARY, &["reqwest"]),
        ("reqwest", &[]),
        ("thiserror", &[]),
    ]);
    assert!(!severed.closure("connector-cli").contains(HOST_LIBRARY));
    assert_eq!(severed.path_to("connector-cli", HOST_LIBRARY), None);
}
