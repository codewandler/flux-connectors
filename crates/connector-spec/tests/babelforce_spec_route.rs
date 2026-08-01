//! Babelforce is compiled through the spec route, and the nine operations it ships do not move —
//! C-416.
//!
//! This is the epic's go/no-go, stated as a test. `providers/babelforce.toml` was hand-authored
//! from the start and its header has said since C-17 that "when the spec lands, this file becomes
//! `[spec]` plus a `[[patch.operations]]` selection and the operation set below is the selection to
//! reproduce". C-415 vendored the document; this file is that sentence made checkable.
//!
//! # What is asserted, and what deliberately is not
//!
//! The **operation contract** is asserted exactly: nine op ids, each with its method and path. Those
//! three are the public surface — a user or a model calls the id, and the host sends the method to
//! the path — so they must survive the front-end change unchanged or the conversion has not
//! reproduced anything.
//!
//! The operations' **parameters and response schemas are not** asserted here, and that is not an
//! oversight. They *did* move, in both directions, and `docs/stories/C-416-reproduce-the-nine.md`
//! records each difference with the evidence for it. Pinning them here would freeze a shape the
//! vendor owns and turn every legitimate re-vendor into a red test — the same reason
//! `vendored_specs.rs` refuses to assert an operation count.
//!
//! It reads `providers/babelforce.toml` and the vendored document off disk rather than embedding
//! either, for the reason `shipped_providers.rs` does: a copy here would be the thing under test
//! drifting away from the thing that ships.

use std::path::{Path, PathBuf};

use connector_spec::{provider, HttpMethod, SpecDocument};

/// The document `[spec] path` must pin, spelled as the provider file spells it.
const PINNED: &str = "specs/babelforce/manager-2026-07-10.openapi.yaml";

/// The nine operations babelforce ships, as `(op id, method, path)`.
///
/// A literal, and it has to be: the claim is that these nine do not move when the front-end changes
/// underneath them, and a set derived from the file under test would agree with whatever that file
/// happens to say. Ordered as the connector publishes them.
const SHIPPED: [(&str, HttpMethod, &str); 9] = [
    ("babelforce-agent-list", HttpMethod::Get, "/api/v2/agents"),
    (
        "babelforce-agent-get",
        HttpMethod::Get,
        "/api/v2/agents/{id}",
    ),
    (
        "babelforce-agent-status-update",
        HttpMethod::Put,
        "/api/v2/agents/{id}/status",
    ),
    (
        "babelforce-call-list",
        HttpMethod::Get,
        "/api/v2/calls/reporting",
    ),
    ("babelforce-call-get", HttpMethod::Get, "/api/v2/calls/{id}"),
    (
        "babelforce-call-hangup",
        HttpMethod::Post,
        "/api/v2/calls/{id}/hangup",
    ),
    (
        "babelforce-call-session-set",
        HttpMethod::Put,
        "/api/v2/calls/{id}/session/set",
    ),
    (
        "babelforce-session-get",
        HttpMethod::Get,
        "/api/v2/sessions/{id}",
    ),
    (
        "babelforce-session-update",
        HttpMethod::Put,
        "/api/v2/sessions/{id}",
    ),
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    let path = repo().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// The five vendored babelforce documents, named one by one.
///
/// A literal rather than a directory read, for the same reason `vendored_specs.rs` spells its own
/// set out: the claim is "these five, and the pin picks the manager one", which a walk of the
/// directory could not make — it would agree with whatever is in there. It also keeps this file
/// inside C-230's rule, which `crates/connector-cli/tests/per_provider_test_scope.rs` enforces
/// textually on `read_dir` in any per-provider test file.
const CACHE: [&str; 5] = [
    "specs/babelforce/auth-2026-06-25.openapi.yaml",
    "specs/babelforce/manager-2026-07-10.openapi.yaml",
    "specs/babelforce/task-automation-2026-06-25.openapi.yaml",
    "specs/babelforce/task-schedule-2026-06-25.openapi.yaml",
    "specs/babelforce/user-2026-06-25.openapi.yaml",
];

/// The shipped provider file, compiled against the whole vendored babelforce spec cache.
///
/// The **cache**, not the pinned document alone: which of the five is compiled is `[spec] path`'s
/// decision, and handing the loader one file would be this test making it instead.
fn load() -> provider::LoadedProvider {
    let cache: Vec<(String, String)> = CACHE
        .iter()
        .map(|path| ((*path).to_owned(), read(path)))
        .collect();
    let documents: Vec<SpecDocument<'_>> = cache
        .iter()
        .map(|(path, document)| SpecDocument {
            path: path.as_str(),
            document: document.as_str(),
        })
        .collect();

    provider::load_with_spec(
        "providers/babelforce.toml",
        &read("providers/babelforce.toml"),
        &documents,
    )
    .unwrap_or_else(|error| panic!("providers/babelforce.toml does not load: {error}"))
}

/// **The story's first acceptance bullet.** The connector is compiled from the vendored document,
/// and it is compiled from the one the file names.
#[test]
fn babelforce_is_compiled_from_the_vendored_manager_document() {
    let loaded = load();

    let spec = loaded.spec.as_ref().unwrap_or_else(|| {
        panic!(
            "providers/babelforce.toml declares no `[spec]` block, so it is still hand-authored. \
             C-415 vendored the document it was waiting for; the file's own header says that is \
             when it becomes `[spec]` plus a `[[patch.operations]]` selection"
        )
    });
    assert_eq!(
        spec.path, PINNED,
        "`[spec] path` pins a different document than the manager one the nine operations come from"
    );

    // Not merely declared — *ingested*. `load_with_spec` resolves the pin against the cache and
    // checks the declared `sha256` against the bytes it actually read, so a populated `ingested`
    // is the evidence that the pin resolved and the hash agreed.
    let ingested = loaded.ingested.as_ref().unwrap_or_else(|| {
        panic!("`[spec]` is declared but no document was ingested, so the pin resolved to nothing")
    });
    assert!(
        ingested.operations.len() > 300,
        "the manager document declares 356 operations and ingest read only {}; the pin is \
         resolving to some other document",
        ingested.operations.len()
    );
}

/// **The reproduction itself.** Nine operations, their ids, methods and paths unchanged.
#[test]
fn the_spec_route_reproduces_the_nine_shipped_operations() {
    let connector = load().connector;

    let published: Vec<(&str, HttpMethod, &str)> = connector
        .operations
        .iter()
        .map(|operation| {
            (
                operation.id.as_str(),
                operation.method,
                operation.path.as_str(),
            )
        })
        .collect();

    assert_eq!(
        published,
        SHIPPED.to_vec(),
        "the spec route does not publish the nine operations babelforce ships. An op id is a public \
         contract users and models call by name, so a difference here is a break, not a rename"
    );
}

/// Every selected operation reaches the IR through a `[[patch.operations]]` entry, and there are
/// exactly nine of them.
///
/// Selection is opt-in and this is what proves the conversion did not quietly widen it: the manager
/// document makes 351 operations available and the connector must publish nine.
#[test]
fn selection_stays_opt_in_and_names_exactly_nine() {
    let loaded = load();

    assert_eq!(
        loaded.patch.operations.len(),
        SHIPPED.len(),
        "the patch set selects {} operations; babelforce ships {}",
        loaded.patch.operations.len(),
        SHIPPED.len()
    );

    let renamed: Vec<&str> = loaded
        .patch
        .operations
        .iter()
        .map(|patch| {
            patch
                .rename
                .as_deref()
                .unwrap_or_else(|| panic!("patch for {:?} states no `rename`", patch.select))
        })
        .collect();
    let shipped: Vec<&str> = SHIPPED.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(
        renamed, shipped,
        "the patch set's `rename`s are not the nine shipped op ids"
    );
}

/// The `SCHEMA GAP` the file's header recorded is closed: provenance is reachable, so the document's
/// identity is a declared, checked `sha256` rather than a sentence in a comment.
///
/// `providers/babelforce.toml:17` said there was "no way to record 'derived from manager.openapi
/// 0.7.0, sha256 6a79679…' for a hand-authored connector". `load_with_spec` refuses a declared hash
/// that disagrees with the bytes, so reaching this assertion at all means the recorded hash is a
/// measurement.
#[test]
fn the_documents_identity_is_recorded_and_checked() {
    let loaded = load();
    let spec = loaded
        .spec
        .as_ref()
        .expect("babelforce is spec-backed; the previous test says so first");

    let declared = spec.sha256.as_deref().unwrap_or_else(|| {
        panic!(
            "`[spec]` declares no `sha256`. Provenance being unreachable was the whole reason the \
             `SCHEMA GAP` comment existed, and an undeclared hash reaches `connectors.lock` as \
             nothing at all"
        )
    });
    assert_eq!(
        declared,
        connector_spec::sha256_hex(read(PINNED).as_bytes()),
        "`[spec] sha256` does not hash the vendored bytes"
    );
}

/// The verification operation still resolves to an operation this connector publishes.
///
/// `verify = "babelforce-agent-list"` is a cross-reference into the operation set, and the
/// conversion moved that set from inline blocks to patch selections. Validation already refuses a
/// dangling `verify`, so this is a guard against the conversion satisfying validation by dropping
/// the field.
#[test]
fn the_verification_operation_survives_the_conversion() {
    let connector = load().connector;

    assert_eq!(
        connector.verify.as_deref(),
        Some("babelforce-agent-list"),
        "the connector's `verify` moved; it is the read that proves a credential works"
    );
}
