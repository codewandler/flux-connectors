//! **The parity claim, made checkable** — C-417.
//!
//! `providers/babelforce.toml` claims to cover the surface manager-sdk covers. A claim like that is
//! worth exactly as much as the thing that refuses it, and this file is that thing: it reads the
//! five vendored documents, counts every operation they declare, compares that against what the
//! connector actually emits, and **refuses a gap that carries no reason**.
//!
//! # Why a set difference rather than a count
//!
//! A count says a number moved. It does not say *which* operation left, and the two failures a
//! coverage gate exists to catch are both invisible to it: a selector whose `path_prefix` stops
//! matching after an upstream reshuffle drops a hundred operations and gains a hundred elsewhere,
//! and a document that renames a path swaps one operation for another at a constant total. So the
//! comparison here is between two **sets of `METHOD path`**, and the difference is enumerated in
//! the failure message — a reviewer reads what went missing, not that something did.
//!
//! # Where "declared" comes from, and why it is not just what ingest published
//!
//! An operation the documents declare reaches this test by one of two routes:
//!
//! - it is in [`Ingested::operations`], because ingest could express it; or
//! - it is in a [`Diagnostic`] whose location names an operation, because ingest **skipped** it.
//!
//! Both are counted as declared, and that second half is the load-bearing one. Without it, an
//! operation that ingest quietly stopped being able to express would vanish from *both* sides of
//! the comparison and the gate would stay green while coverage fell — which is precisely the
//! regression this story was asked to make impossible. It is not hypothetical: five operations sit
//! in that bucket today.
//!
//! # The allow-list is the whole design
//!
//! Every gap must be named in [`ALLOWED`], **with a reason**, and every [`ALLOWED`] entry must
//! correspond to a real gap. Both directions, because a one-directional check rots in the easy
//! direction: an entry left behind after the gap it explained was closed is an entry that would
//! silently excuse the *next* operation to go missing under the same path.

use std::collections::BTreeSet;

use connector_spec::{HttpMethod, Idempotency, LoadedProvider, Risk};

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

/// **The canonical surface manager-sdk covers.**
///
/// Owner-stated, 2026-08-01: *"only include things which are also included in manager-sdk itself"*.
/// The five documents declare [`DECLARED`] operations; the canonical surface is that less
/// `POST /api/v1/webhook/zendesk`, a receiver babelforce exposes for Zendesk to call rather than an
/// operation a client invokes.
const CANONICAL: usize = 397;

/// Everything the five vendored documents declare, receiver included.
///
/// This is a claim about the *documents*, so a re-vendor that moves it is expected to move this
/// number in the same commit — and to say, in the story that moved it, what the vendor added.
const DECLARED: usize = 398;

/// **What the connector emits, and the reason it is not [`CANONICAL`].**
///
/// Ingest skips an operation whose request body is `multipart/form-data`, with a diagnostic, rather
/// than publishing it without its body: `BodyEncoding` is `Json | Form` and this IR has no third
/// value. Five manager operations are file uploads.
///
/// So `392 + 5 = 397`. That is an **IR gap, not a selection gap** — nothing here failed to match
/// them, ingest never produced them — and closing it is C-426's. When it closes, this constant
/// rises to [`CANONICAL`] and five [`ALLOWED`] entries are deleted, in one commit, together.
const EMITTED: usize = 392;

/// The operations the connector deliberately does not emit, each with the reason it does not.
///
/// `(method, path, reason)`. The reason is not decoration: a gap with no sentence beside it is a
/// coverage hole nobody decided on, and [`the_documents_are_covered_and_every_gap_carries_a_reason`]
/// refuses one. It is also refused at a length that would let `"n/a"` through — see
/// [`MIN_REASON`].
const ALLOWED: [(HttpMethod, &str, &str); 6] = [
    (
        HttpMethod::Post,
        "/api/v1/webhook/zendesk",
        "a webhook receiver Zendesk calls into babelforce, not a call flux makes out of it. \
         Outside manager-sdk's surface, and excluded by the task-automation selectors stating \
         `/api/v3` rather than by naming it",
    ),
    (
        HttpMethod::Post,
        "/api/v2/agents/provision",
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 closes this",
    ),
    (
        HttpMethod::Post,
        "/api/v2/agents/provision/validate",
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 closes this",
    ),
    (
        HttpMethod::Post,
        "/api/v2/outbound/lists/{id}/leads/upload",
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 closes this",
    ),
    (
        HttpMethod::Post,
        "/api/v2/phonebook/bulk",
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 closes this",
    ),
    (
        HttpMethod::Post,
        "/api/v2/prompts",
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 closes this",
    ),
];

/// The shortest string that counts as a reason, in characters after trimming.
///
/// Calibrated the same way [`connector_spec::MIN_REPEATABILITY_CONDITION`] is: it is a floor on
/// effort rather than a measure of truth. Below it live `"n/a"`, `"skipped"` and `"see above"`,
/// which satisfy the allow-list while telling a reviewer nothing — and an escape hatch that costs
/// nothing is a deleted guard.
const MIN_REASON: usize = 24;

/// The nine operations that reach a model as tools, and the full contract each publishes.
///
/// `(op id, method, path, risk, idempotency)`. A literal, and it has to be: the claim is that these
/// nine do not move when the surface around them grows by a factor of forty, and a set derived from
/// the file under test would agree with whatever that file happens to say.
///
/// **`risk` and `idempotency` are here, not only the id, and that is what this file adds to
/// `babelforce_spec_route.rs`.** Widening to 392 operations meant declaring a blunt
/// `risk = "high"` over every manager write, and three of these nine *are* manager writes that ship
/// as `medium`/`idempotent`. Those two fields reach a host's approval gate and its retry decision,
/// so letting a bulk selector raise them would have been a silent behavioural change to three
/// shipped tools dressed as a refactor. This is the assertion that would have caught it.
const EXPOSED: [(&str, HttpMethod, &str, Risk, Idempotency); 9] = [
    (
        "babelforce-agent-list",
        HttpMethod::Get,
        "/api/v2/agents",
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "babelforce-agent-get",
        HttpMethod::Get,
        "/api/v2/agents/{id}",
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "babelforce-agent-status-update",
        HttpMethod::Put,
        "/api/v2/agents/{id}/status",
        Risk::Medium,
        Idempotency::Idempotent,
    ),
    (
        "babelforce-call-list",
        HttpMethod::Get,
        "/api/v2/calls/reporting",
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "babelforce-call-get",
        HttpMethod::Get,
        "/api/v2/calls/{id}",
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "babelforce-call-hangup",
        HttpMethod::Post,
        "/api/v2/calls/{id}/hangup",
        Risk::Destructive,
        Idempotency::NonIdempotent,
    ),
    (
        "babelforce-call-session-set",
        HttpMethod::Put,
        "/api/v2/calls/{id}/session/set",
        Risk::Medium,
        Idempotency::Idempotent,
    ),
    (
        "babelforce-session-get",
        HttpMethod::Get,
        "/api/v2/sessions/{id}",
        Risk::Low,
        Idempotency::Idempotent,
    ),
    (
        "babelforce-session-update",
        HttpMethod::Put,
        "/api/v2/sessions/{id}",
        Risk::Medium,
        Idempotency::Idempotent,
    ),
];

/// How a diagnostic's `location` spells a method, and how this file keys an operation.
///
/// `openapi::Located::location` renders `GET /api/v2/agents`, so an operation skipped by ingest and
/// an operation it published have to be spelled the same way to sit in one set. There is no
/// `Display` for [`HttpMethod`] to borrow, so the mapping lives here, closed over every variant —
/// a `_` arm would let a new method silently key as something else.
fn word(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Head => "HEAD",
        HttpMethod::Options => "OPTIONS",
    }
}

/// `GET /api/v2/agents` — the key every set in this file is built from.
fn key(method: HttpMethod, path: &str) -> String {
    format!("{} {path}", word(method))
}

/// Every method word a diagnostic location may open with, including the two the IR cannot emit.
///
/// `OPTIONS` and `TRACE` are here because `openapi::UNREPRESENTABLE_METHODS` diagnoses an operation
/// declared under either, and such an operation is still something the document *declares*. Zero
/// occurrences across this corpus, which is the point: if one arrives it must land in the gap and
/// need a reason, not be filtered out of the accounting by a list that forgot it.
const METHOD_WORDS: [&str; 8] = [
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE",
];

fn load() -> LoadedProvider {
    shipped_provider::load("babelforce")
}

/// **Everything the five documents declare**, whether or not ingest could express it.
///
/// The union of what ingest published and what it diagnosed away. See the module docs for why the
/// second half cannot be dropped.
fn declared(loaded: &LoadedProvider) -> BTreeSet<String> {
    let mut declared: BTreeSet<String> = BTreeSet::new();

    for document in &loaded.ingested {
        for operation in &document.ingested.operations {
            declared.insert(key(operation.method, &operation.path));
        }
        for diagnostic in &document.ingested.diagnostics {
            // A diagnostic about the document as a whole (`servers`, `paths`) is not about an
            // operation and must not be counted as one. The ones that are open with a method word.
            let Some((head, path)) = diagnostic.location.split_once(' ') else {
                continue;
            };
            if METHOD_WORDS.contains(&head) && path.starts_with('/') {
                declared.insert(diagnostic.location.clone());
            }
        }
    }

    declared
}

/// What the connector publishes.
fn emitted(loaded: &LoadedProvider) -> BTreeSet<String> {
    loaded
        .connector
        .operations
        .iter()
        .map(|operation| key(operation.method, &operation.path))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------------------------

/// **The story's coverage gate.** Every operation the documents declare is either emitted or
/// allow-listed with a reason, and every allow-list entry explains a gap that really exists.
#[test]
fn the_documents_are_covered_and_every_gap_carries_a_reason() {
    let loaded = load();
    let declared = declared(&loaded);
    let emitted = emitted(&loaded);

    assert_eq!(
        declared.len(),
        DECLARED,
        "the five vendored documents declare {} operations, not {DECLARED}. A re-vendor moving \
         this is expected to move the constant in the same commit and say what the vendor added",
        declared.len()
    );

    // Nothing is emitted that no document declares. Cheap, and it is the assertion that would catch
    // a connector accreting a hand-authored operation beside a spec-backed surface.
    let invented: Vec<&String> = emitted.difference(&declared).collect();
    assert!(
        invented.is_empty(),
        "the connector emits operations no vendored document declares: {invented:?}"
    );

    let allowed: BTreeSet<String> = ALLOWED
        .iter()
        .map(|(method, path, _)| key(*method, path))
        .collect();
    let gap: BTreeSet<String> = declared.difference(&emitted).cloned().collect();

    let unexplained: Vec<&String> = gap.difference(&allowed).collect();
    assert!(
        unexplained.is_empty(),
        "{} operations the documents declare are not emitted and carry no reason:\n  {}\n\nAdd \
         each to `ALLOWED` with the reason it is absent, or widen the selectors in \
         `providers/babelforce.toml` until it is emitted. A gap nobody wrote a sentence about is a \
         coverage hole nobody decided on",
        unexplained.len(),
        unexplained
            .iter()
            .map(|entry| entry.as_str())
            .collect::<Vec<_>>()
            .join("\n  "),
    );

    // The other direction. An entry outliving the gap it explained would silently excuse the next
    // operation to go missing under the same path.
    let stale: Vec<&String> = allowed.difference(&gap).collect();
    assert!(
        stale.is_empty(),
        "`ALLOWED` explains gaps that no longer exist: {stale:?}. Delete the entry — an allow-list \
         entry with nothing to allow is a pre-authorised regression"
    );

    for (method, path, reason) in ALLOWED {
        assert!(
            reason.trim().chars().count() >= MIN_REASON,
            "the reason for {} is {} characters, and a reason shorter than {MIN_REASON} tells a \
             reviewer nothing: {reason:?}",
            key(method, path),
            reason.trim().chars().count(),
        );
    }

    // **The accounting, stated as arithmetic rather than as prose.** `392 + 5 = 397`, and the
    // sixth allow-list entry is the receiver, which is outside the canonical surface rather than
    // missing from it.
    assert_eq!(emitted.len(), EMITTED, "the emitted operation count moved");
    assert_eq!(
        emitted.len() + (ALLOWED.len() - 1),
        CANONICAL,
        "{EMITTED} emitted + {} inexpressible = the {CANONICAL} manager-sdk covers",
        ALLOWED.len() - 1
    );
    assert_eq!(
        CANONICAL + 1,
        DECLARED,
        "the canonical surface is everything the documents declare less the webhook receiver"
    );
}

/// **No `internal` path segment, ever** — the guard, not a filter.
///
/// Measured 2026-08-01: `internal` appears in zero paths across all five documents, so there is
/// nothing to exclude today. That is exactly why this is here. A future spec pull that introduces
/// one would otherwise be swept in by a `path_prefix` selector that already matches its parent,
/// silently, and an internal surface is not something a tenant's credential should be able to reach
/// because a prefix happened to cover it.
#[test]
fn no_emitted_operation_lives_on_an_internal_path() {
    let connector = load().connector;

    let internal: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| {
            operation
                .path
                .split('/')
                .any(|segment| segment == "internal")
        })
        .map(|operation| operation.path.as_str())
        .collect();

    assert!(
        internal.is_empty(),
        "operations on an `internal` path reached the connector: {internal:?}. Narrow the \
         selectors in `providers/babelforce.toml`; an internal surface is not part of the \
         manager-sdk scope and never becomes so by a prefix widening underneath it"
    );
}

// ---------------------------------------------------------------------------------------------
// Exposure — the reason 392 operations is survivable
// ---------------------------------------------------------------------------------------------

/// **Nine tools, not 392.** The exposure tier's whole purpose, asserted as a number.
///
/// C-413 split "callable" from "exposed" precisely so that widening a connector to a vendor's full
/// surface does not spend a model's entire context on a tool list. Without this assertion the
/// mechanism is one absent `expose = false` away from being undone — a selector that loses the key
/// still compiles, still emits every operation, and turns the catalogue into 392 LLM tools.
#[test]
fn only_the_curated_nine_reach_a_model() {
    let connector = load().connector;

    let exposed: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| operation.expose)
        .map(|operation| operation.id.as_str())
        .collect();
    let expected: Vec<&str> = EXPOSED.iter().map(|(id, _, _, _, _)| *id).collect();

    assert_eq!(
        exposed, expected,
        "the exposed set is not the curated nine. 392 operations are callable and nine are tools; \
         a difference here is either a tool that vanished from every caller's reach or several \
         hundred that arrived in a model's context"
    );

    assert_eq!(
        connector
            .operations
            .iter()
            .filter(|operation| !operation.expose)
            .count(),
        EMITTED - EXPOSED.len(),
        "everything past the curated nine is catalogued and callable without reaching a model"
    );
}

/// **The nine shipped operations are a public contract, and this is the whole of it.**
///
/// Id, method, path, risk and idempotency. Users and models call the id; the host sends the method
/// to the path; the approval gate reads the risk and the retry decision reads the idempotency. All
/// five have to survive the widening or it broke something while claiming to add.
#[test]
fn the_nine_shipped_operations_keep_their_contract() {
    let connector = load().connector;

    for (id, method, path, risk, idempotency) in EXPOSED {
        let published = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| {
                panic!(
                    "`{id}` is no longer published. An op id is a public contract users and models \
                     call by name, so this is a break, not a rename"
                )
            });

        assert_eq!(
            (published.method, published.path.as_str()),
            (method, path),
            "`{id}` no longer sends {} {path}",
            word(method)
        );
        assert_eq!(
            (published.risk, published.idempotency),
            (risk, idempotency),
            "`{id}` ships as {risk:?}/{idempotency:?} and now publishes \
             {:?}/{:?}. Both reach a host's approval gate and its retry decision — if a bulk \
             `[[patch.select]]` raised them, state the shipped values in that operation's own \
             `[[patch.operations]]` block",
            published.risk,
            published.idempotency,
        );
    }

    // Order, too: the nine publish first and in the file's block order, which is what
    // `connectors/babelforce.flux` renders and therefore what a reviewer diffs.
    let leading: Vec<&str> = connector
        .operations
        .iter()
        .take(EXPOSED.len())
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        leading,
        EXPOSED
            .iter()
            .map(|(id, _, _, _, _)| *id)
            .collect::<Vec<_>>(),
        "the curated nine no longer lead the published set"
    );
}

/// **A tool contract with no sentence in it does not ship.**
///
/// 23 operations across the five documents declare neither `summary` nor `description`. The story's
/// rule is that each either gets one through the overlay or stays unexposed — and since 383 of the
/// 392 are unexposed, that is very nearly free. This is the assertion that keeps it free rather
/// than assumed: the day someone exposes one of the 23, the build fails until they write the
/// sentence a model would otherwise have to guess from.
#[test]
fn every_exposed_operation_carries_a_sentence() {
    let connector = load().connector;

    let silent: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| operation.expose)
        .filter(|operation| operation.description.trim().is_empty())
        .map(|operation| operation.id.as_str())
        .collect();

    assert!(
        silent.is_empty(),
        "exposed operations carry no description: {silent:?}. Their documents declare neither \
         `summary` nor `description`, so a `description` in the operation's `[[patch.operations]]` \
         block is the only place the sentence can come from. An LLM tool with no description is a \
         contract with nothing in it"
    );
}

/// The story's number, measured — 23 undescribed operations, and all of them unexposed.
///
/// Measured rather than assumed, because "this should be nearly free" is the kind of sentence that
/// is true right up until it is not. The count is pinned for the same reason [`DECLARED`] is: it is
/// a claim about the vendored documents, so a re-vendor that changes it should say so rather than
/// absorb it — and a *rising* count is the signal worth having, since each new one is an operation
/// that cannot be exposed without someone writing its sentence first.
///
/// All 23 are `task-automation` and `task-schedule` operations, which is not a coincidence: those
/// two documents are generated from a service whose authors did not write summaries, and they are
/// the two whose `info.version` the vendor actually maintains.
const UNDESCRIBED: usize = 23;

#[test]
fn the_undescribed_operations_are_all_unexposed() {
    let connector = load().connector;

    let undescribed: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| operation.description.trim().is_empty())
        .map(|operation| operation.id.as_str())
        .collect();

    assert_eq!(
        undescribed.len(),
        UNDESCRIBED,
        "{} emitted operations declare neither `summary` nor `description`, not {UNDESCRIBED}. \
         Either the documents moved, or the selectors stopped reaching those operations: \
         {undescribed:?}",
        undescribed.len()
    );
    assert!(
        undescribed
            .iter()
            .all(|id| !EXPOSED.iter().any(|(exposed, _, _, _, _)| exposed == id)),
        "an operation with no description is exposed to a model: {undescribed:?}"
    );
}
