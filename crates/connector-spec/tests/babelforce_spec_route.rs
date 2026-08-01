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

use connector_spec::{HttpMethod, LoadedProvider};

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

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

fn read(relative: &str) -> String {
    let path = shipped_provider::root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// The shipped provider file, compiled against the whole vendored babelforce spec cache.
///
/// Through C-421's shared seam rather than a cache assembled here. That is the point of the seam:
/// it passes **every** document under `specs/babelforce/` and lets `[spec] path` resolve the pin,
/// so this file states nothing about which of the five is compiled — which is the only way the
/// claims below stay claims about the provider rather than about a cache the test picked.
fn load() -> LoadedProvider {
    shipped_provider::load("babelforce")
}

/// **The story's first acceptance bullet.** The connector is compiled from the vendored document,
/// and it is compiled from the one the file names.
#[test]
fn babelforce_is_compiled_from_the_vendored_manager_document() {
    let loaded = load();

    // Exactly one, and it is the manager document. C-410 made `specs` a list, so "which documents
    // does this connector compile from" is now a claim worth stating rather than a shape the type
    // guarantees: the other four vendored babelforce documents are in the cache this was loaded
    // against, and none of them may reach the connector until a story selects out of it.
    let pinned: Vec<&str> = loaded.specs.iter().map(|spec| spec.path.as_str()).collect();
    assert_eq!(
        pinned,
        vec![PINNED],
        "providers/babelforce.toml does not pin exactly the manager document. If it declares no \
         `[spec]` at all it is still hand-authored — C-415 vendored the document it was waiting \
         for, and the file's own header says that is when it becomes `[spec]` plus a \
         `[[patch.operations]]` selection"
    );

    // Not merely declared — *ingested*. `load_with_spec` resolves each pin against the cache and
    // checks the declared `sha256` against the bytes it actually read, so a populated `ingested`
    // is the evidence that the pin resolved and the hash agreed.
    let [document] = loaded.ingested.as_slice() else {
        panic!(
            "expected exactly one ingested document, got {}",
            loaded.ingested.len()
        )
    };
    assert_eq!(document.path, PINNED);
    assert!(
        document.ingested.operations.len() > 300,
        "the manager document declares 356 operations and ingest read only {}; the pin is \
         resolving to some other document",
        document.ingested.operations.len()
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
    let [spec] = loaded.specs.as_slice() else {
        panic!("babelforce pins exactly one document; the previous test says so first")
    };

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
