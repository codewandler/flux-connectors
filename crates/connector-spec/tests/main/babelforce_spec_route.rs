//! Babelforce is compiled through the spec route, and the nine operations it ships do not move —
//! C-416.
//!
//! This is the epic's go/no-go, stated as a test. `providers/babelforce.toml` was hand-authored
//! from the start and its header has said since C-17 that "when the spec lands, this file becomes
//! `[spec]` plus a `[[patch.operations]]` selection and the operation set below is the selection to
//! reproduce". C-415 vendored the document; this file is that sentence made checkable.
//!
//! # What C-417 changed here, and what it did not
//!
//! The connector now compiles from **four** of the five vendored documents rather than one, and publishes 388 operations
//! rather than nine. Every assertion below that counted has been restated as the claim it was
//! standing in for, because each of them was a proxy for "the conversion did not widen selection by
//! accident" and that sentence stopped being the question the day widening became the goal.
//!
//! What did **not** change is the thing this file exists for: the nine ids, methods and paths are a
//! public contract and they are asserted exactly. `babelforce_coverage.rs` carries the rest of that
//! contract — risk, idempotency and exposure — and the coverage accounting the widening rests on.
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

use crate::shipped_provider;

/// The manager document, which carries the nine and is the one this file's claims are about.
const PINNED: &str = "specs/babelforce/manager-2026-07-10.openapi.yaml";

/// **Every document `[[spec]]` must pin, as `(service, path)`** — C-410, C-417.
///
/// Spelled here rather than globbed. `specs/babelforce/` is a *cache*, and which of its files a
/// connector compiles from is the provider file's decision — so a test that discovered its own
/// inputs would agree with whatever the file happened to say, which is the one thing it must not do.
/// Ordered as `providers/babelforce.toml` declares them, because that order is what decides the
/// order operations publish in.
///
/// **Four, not the five that are vendored.** `auth-2026-06-25.openapi.yaml` is deliberately not
/// pinned here: all three of its endpoints are withheld because an authentication endpoint is never
/// an operation (`AGENTS.md` § Authentication contract), which would leave the `auth` service
/// carrying zero operations — and `services.rs` refuses a service that emits an empty module, while
/// the loader refuses a `[[spec]]` naming an undeclared service. The document is still vendored and
/// still hash-checked, through `specs/babelforce.provenance.toml` and `vendored_specs.rs`.
const DOCUMENTS: [(&str, &str); 4] = [
    ("manager", PINNED),
    ("user", "specs/babelforce/user-2026-06-25.openapi.yaml"),
    (
        "task-automation",
        "specs/babelforce/task-automation-2026-06-25.openapi.yaml",
    ),
    (
        "task-schedule",
        "specs/babelforce/task-schedule-2026-06-25.openapi.yaml",
    ),
];

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
/// so this file states nothing about which of the four is compiled — which is the only way the
/// claims below stay claims about the provider rather than about a cache the test picked.
fn load() -> LoadedProvider {
    shipped_provider::load("babelforce")
}

/// **The story's first acceptance bullet.** The connector is compiled from the vendored documents,
/// and from exactly the ones the file names.
#[test]
fn babelforce_is_compiled_from_the_pinned_vendored_documents() {
    let loaded = load();

    // **All four pinned, each joined to its service.** This asserted `vec![PINNED]` while babelforce
    // compiled from one document, and it was a real claim then: the other four were in the cache
    // and none of them could reach the connector until a story selected out of one. C-417 is that
    // story, so the claim is restated at its new value rather than relaxed — a connector that
    // dropped a document would still fail here, which is the property worth keeping.
    let pinned: Vec<(&str, &str)> = loaded
        .specs
        .iter()
        .map(|spec| (spec.service(), spec.path.as_str()))
        .collect();
    assert_eq!(
        pinned,
        DOCUMENTS.to_vec(),
        "providers/babelforce.toml does not pin exactly the four vendored documents it reads, one per \
         service. If it declares no `[spec]` at all it is still hand-authored"
    );

    // Not merely declared — *ingested*. `load_with_spec` resolves each pin against the cache and
    // checks the declared `sha256` against the bytes it actually read, so a populated `ingested`
    // is the evidence that every pin resolved and every hash agreed.
    let ingested: Vec<&str> = loaded
        .ingested
        .iter()
        .map(|document| document.path.as_str())
        .collect();
    assert_eq!(
        ingested,
        DOCUMENTS.map(|(_, path)| path).to_vec(),
        "a pinned document was not ingested"
    );

    let manager = loaded
        .ingested
        .iter()
        .find(|document| document.path == PINNED)
        .expect("the manager document is ingested");
    assert!(
        manager.ingested.operations.len() > 300,
        "the manager document declares 356 operations and ingest read only {}; the pin is \
         resolving to some other document",
        manager.ingested.operations.len()
    );
}

/// **The reproduction itself.** Nine operations, their ids, methods and paths unchanged.
///
/// **The nine lead the published set**, and that is asserted rather than the whole set being
/// compared. The order is not an accident to be tolerated: a `[[patch.operations]]` block publishes
/// before anything a selector matched, and the nine are the only blocks that name a manager
/// operation, so a slice of the front is exactly as strong a statement as the old whole-list
/// equality — and it goes on saying it while the tail grows.
#[test]
fn the_spec_route_reproduces_the_nine_shipped_operations() {
    let connector = load().connector;

    let published: Vec<(&str, HttpMethod, &str)> = connector
        .operations
        .iter()
        .take(SHIPPED.len())
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

/// **Selection is still opt-in, and the nine are still named one at a time.**
///
/// This asserted `patch.operations.len() == 9` while nine was the whole connector, and the sentence
/// it was standing in for was "the conversion did not quietly widen selection". C-417 widened it on
/// purpose, through `[[patch.select]]` — so the claim is restated as the two things that did not
/// change: every operation still arrives through a *declaration* rather than by default, and each
/// of the nine is still named by a block of its own rather than swept into a set.
///
/// The tenth block is `getUser`, which two documents declare and which therefore cannot be named by
/// a `[patch.naming.pin]` at all.
#[test]
fn selection_stays_opt_in_and_the_nine_are_named_one_at_a_time() {
    let loaded = load();

    // Opt-in: an operation reaches the connector only because a selector or a block said so.
    // Emptying both would empty the connector, which is the property that lets a 398-operation
    // corpus be pointed at safely.
    assert!(
        !loaded.patch.select.is_empty(),
        "no `[[patch.select]]` statement, so the 388 operations must have arrived by default — \
         which is the one thing selection must never do"
    );

    // Each of the nine, held by a block **and** by a pin. `rename` is no longer where the id is
    // stated: `[patch.naming.pin]` is, because a pin is what holds a shipped id still underneath a
    // rule that derives everything around it. Both halves are checked, because either alone is
    // insufficient — a pin with no block would leave the operation unexposed and uncorrected, and a
    // block with no pin would let the naming rule decide a public contract.
    let naming = loaded
        .patch
        .naming
        .as_ref()
        .expect("`[patch.naming]` declares the rule the other 382 ids are derived by");

    for (id, _, _) in SHIPPED {
        let pinned: Vec<&str> = naming
            .pin
            .iter()
            .filter(|(_, published)| published.as_str() == id)
            .map(|(operation_id, _)| operation_id.as_str())
            .collect();
        let [operation_id] = pinned.as_slice() else {
            panic!(
                "`{id}` is published by {} pins, not one; a shipped op id left to the naming rule \
                 is one upstream `operationId` rename away from moving",
                pinned.len()
            )
        };

        assert!(
            loaded
                .patch
                .operations
                .iter()
                .any(|patch| patch.select == *operation_id
                    && patch.service.as_deref() == Some("manager")),
            "`{operation_id}` is pinned to `{id}` and has no `[[patch.operations]]` block in the \
             manager service, so nothing states its exposure or its corrections"
        );
    }
}

/// The `SCHEMA GAP` the file's header recorded is closed: provenance is reachable, so each
/// document's identity is a declared, checked `sha256` rather than a sentence in a comment.
///
/// `providers/babelforce.toml:17` said there was "no way to record 'derived from manager.openapi
/// 0.7.0, sha256 6a79679…' for a hand-authored connector". `load_with_spec` refuses a declared hash
/// that disagrees with the bytes, so reaching this assertion at all means every recorded hash is a
/// measurement.
///
/// **Per document, which is the reason `[[spec]]` is a list** (C-410): the five were pulled on two
/// different dates and three of them publish `info.version = "0.0.0-dev"`, so one hash for the
/// connector could not answer the only question a drift check is asked — *which* of them moved.
#[test]
fn every_documents_identity_is_recorded_and_checked() {
    let loaded = load();
    assert_eq!(
        loaded.specs.len(),
        DOCUMENTS.len(),
        "babelforce pins {} documents; the first test in this file says which",
        loaded.specs.len()
    );

    for spec in &loaded.specs {
        let declared = spec.sha256.as_deref().unwrap_or_else(|| {
            panic!(
                "`[[spec]]` for {:?} declares no `sha256`. Provenance being unreachable was the \
                 whole reason the `SCHEMA GAP` comment existed, and an undeclared hash reaches \
                 `connectors.lock` as nothing at all",
                spec.path
            )
        });
        assert_eq!(
            declared,
            connector_spec::sha256_hex(read(&spec.path).as_bytes()),
            "`[[spec]] sha256` for {:?} does not hash the vendored bytes",
            spec.path
        );
    }
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
