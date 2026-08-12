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

use connector_spec::{HttpMethod, Idempotency, LoadedProvider, ParamPosition, Risk};

use crate::shipped_provider;

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

/// C-30: every vendor-declared string-or-array query union deliberately publishes its scalar arm.
#[test]
fn ambiguous_query_collections_are_narrowed_to_their_scalar_branch() {
    let loaded = load();
    let narrowed = loaded
        .patch
        .operations
        .iter()
        .flat_map(|operation| {
            operation
                .params
                .iter()
                .filter(|parameter| parameter.position == ParamPosition::Query)
                .map(move |parameter| {
                    assert_eq!(
                        parameter
                            .schema
                            .as_ref()
                            .and_then(|schema| schema["type"].as_str()),
                        Some("string"),
                        "{} query parameter {} was not narrowed to a scalar string",
                        operation.select,
                        parameter.name
                    );
                    (operation.select.as_str(), parameter.name.as_str())
                })
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        narrowed,
        BTreeSet::from([
            ("listAgents", "groupIds"),
            ("listAgents", "groups"),
            ("listAgents", "tags"),
            ("listAllSimpleReportingCalls", "agentId"),
            ("listAllSimpleReportingCalls", "id"),
            ("listAllSimpleReportingCalls", "parentId"),
            ("listAllSimpleReportingCalls", "to"),
            ("listAllSimpleReportingCalls", "toNumber"),
            ("listAllSimpleReportingCalls", "type"),
            ("listDashboards", "uuid"),
            ("listLiveLogs", "filters.level"),
            ("listReportingCalls", "agentId"),
            ("listReportingCalls", "finishReason"),
            ("listReportingCalls", "id"),
            ("listReportingCalls", "state"),
            ("listReportingCalls", "toNumber"),
            ("listReportingCalls", "type"),
            ("listUsers", "email"),
        ]),
        "the explicit scalar-branch inventory changed"
    );
}

/// **The endpoints of the one vendored document the connector no longer reads.**
///
/// `providers/babelforce.toml` has no `[[spec]]` entry for `auth-2026-06-25.openapi.yaml`, because
/// all three of its endpoints are withheld and a service carrying zero operations emits an empty
/// module — which `services.rs` refuses, and the loader refuses a `[[spec]]` whose service is not
/// declared, so the two entries stand or fall together.
///
/// **They are named here rather than dropped**, and that is the whole point of this constant. The
/// module docs above explain why an operation must never vanish from *both* sides of the
/// comparison: the gate would stay green while coverage fell. A document the connector stops
/// reading is exactly that failure with a different cause, so its endpoints are folded back into
/// [`declared`] by name and must still carry an [`ALLOWED`] reason like any other gap.
///
/// Their bytes are still checked: `specs/babelforce.provenance.toml` records this document's
/// `sha256`, and `vendored_specs.rs` verifies it against the file independently of the provider
/// definition.
const UNREAD: [(HttpMethod, &str); 3] = [
    (HttpMethod::Get, "/oauth/authorize"),
    (HttpMethod::Post, "/oauth/revoke"),
    (HttpMethod::Post, "/oauth/token"),
];

/// **What the connector emits, and the two reasons it is not [`CANONICAL`].**
///
/// - **Five cannot be expressed** ([`Gap::Inexpressible`]). Ingest skips an operation whose request
///   body is `multipart/form-data`, with a diagnostic, rather than publishing it without its body:
///   `BodyEncoding` is `Json | Form` and this IR has no third value. Five manager operations are
///   file uploads. That is an **IR gap, not a selection gap** — nothing here failed to match them,
///   ingest never produced them. C-426 went to close it and found the gap is **not this
///   repository's**: flux 0.49 cannot carry a multipart body, so the IR variant would describe a
///   request no emitted module could perform. See the entries themselves.
/// - **Four are withheld by rule** ([`Gap::Withheld`]). Three `/oauth/*` endpoints, because an
///   authentication endpoint describes how to authenticate rather than being an operation; and
///   `GET /api/v2/user/account`, because its 200 body delivers two credentials. Both rules are in
///   `AGENTS.md` § Authentication contract, and the second is explicitly *"not only an OAuth one"*.
///
/// So `388 + 5 + 4 = 397` — **three categories, not two**, because the two kinds of absence are not
/// the same claim: one is a limit and the other is a decision. Reporting them as one number is how
/// a deliberate exclusion comes to read as something missing. When a gap closes, this constant
/// rises and the matching [`ALLOWED`] entries are deleted, in one commit, together — which the
/// stale-entry half of the gate enforces.
const EMITTED: usize = 388;

/// **Why an operation the documents declare is not emitted.** Three kinds, and they are not
/// interchangeable.
///
/// The distinction is the point of the accounting: [`Gap::Inexpressible`] is a *limit* this
/// repository would lift if it could, [`Gap::Withheld`] is a *decision* it would not, and
/// [`Gap::OutsideSurface`] is not a gap in coverage at all. Collapsing them into one count is what
/// lets a withheld endpoint read as a coverage hole, and a coverage hole read as a decision.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gap {
    /// Outside manager-sdk's canonical surface, so not missing from it. The receiver, and the
    /// reason [`CANONICAL`] is 397 rather than [`DECLARED`].
    OutsideSurface,
    /// Inside the surface, and ingest cannot produce it.
    Inexpressible,
    /// Inside the surface and perfectly expressible, withheld on purpose.
    ///
    /// Two rules land here, both from `AGENTS.md` § Authentication contract and both about
    /// credentials rather than about coverage: an authentication endpoint is never an operation,
    /// and an operation whose declared response carries a token is withheld until C-136's
    /// diversion lands. They share a category because they share a consequence — the entry's own
    /// reason says which rule it is.
    Withheld,
}

/// How many [`ALLOWED`] entries carry each [`Gap`], as the accounting states them.
const OUTSIDE_SURFACE: usize = 1;
const INEXPRESSIBLE: usize = 5;
const WITHHELD: usize = 4;

/// The operations the connector deliberately does not emit, each with the reason it does not.
///
/// `(method, path, reason)`. The reason is not decoration: a gap with no sentence beside it is a
/// coverage hole nobody decided on, and [`the_documents_are_covered_and_every_gap_carries_a_reason`]
/// refuses one. It is also refused at a length that would let `"n/a"` through — see
/// [`MIN_REASON`].
///
/// **Three kinds of entry, and they are not interchangeable** — see [`Gap`], which each entry
/// carries so the accounting can count them apart rather than by position in this list.
///
/// The withheld entries state the **general rule** rather than restatements of a special case: an
/// authentication endpoint is never an operation, and an operation whose response delivers a
/// credential waits for C-136. Each per-endpoint sentence says only which rule it is and what that
/// particular path carries.
const ALLOWED: [(HttpMethod, &str, Gap, &str); 10] = [
    (
        HttpMethod::Post,
        "/api/v1/webhook/zendesk",
        Gap::OutsideSurface,
        "a webhook receiver Zendesk calls into babelforce, not a call flux makes out of it. \
         Outside manager-sdk's surface, and excluded by the task-automation selectors stating \
         `/api/v3` rather than by naming it",
    ),
    // ---- Authentication, not operations. AGENTS.md § Authentication contract. ----
    (
        HttpMethod::Post,
        "/oauth/token",
        Gap::Withheld,
        "an authentication endpoint describes how to authenticate and is never a connector \
         operation (owner-stated 2026-08-01, AGENTS.md § Authentication contract): it is the \
         connector's authentication surface, performed by the host, not a call anyone invokes for \
         a result. This one is the token grant itself. **Independently** its 2xx body is \
         `OAuthTokenResponse`, declaring `access_token` and `refresh_token` \
         (auth-2026-06-25.openapi.yaml:260), which no redactor here can reach — the host's \
         redactor holds only values it resolved before the call \
         (connector-pack/src/credentials.rs:149, rendered at connectors-api/src/exec.rs:98). Both \
         reasons are true and the auth-flow one is load-bearing, because it still holds after \
         C-136 lands the credential-response diversion",
    ),
    (
        HttpMethod::Get,
        "/oauth/authorize",
        Gap::Withheld,
        "an authentication endpoint describes how to authenticate and is never a connector \
         operation (owner-stated 2026-08-01, AGENTS.md § Authentication contract). This one is the \
         PKCE browser redirect — `response_type`, `redirect_uri`, `code_challenge`, \
         `code_challenge_method` — an endpoint a user-agent is sent to, with no result to return \
         to a program. It was emitted as `babelforce-authorize` through v0.9.0",
    ),
    (
        HttpMethod::Post,
        "/oauth/revoke",
        Gap::Withheld,
        "an authentication endpoint describes how to authenticate and is never a connector \
         operation (owner-stated 2026-08-01, AGENTS.md § Authentication contract). This one takes \
         a `client_secret` as a plain argument, which is auth-flow material travelling as an \
         operation parameter — the shape the three-axis auth model exists to keep out of a call \
         signature. It was emitted as `babelforce-revoke` through v0.9.0",
    ),
    // ---- Withheld for what it returns, not for what it is. ----
    (
        HttpMethod::Get,
        "/api/v2/user/account",
        Gap::Withheld,
        "its 200 body delivers two credentials, so it is withheld under the same contract as the \
         `/oauth/*` three — AGENTS.md § Authentication contract, which says explicitly `not only an \
         OAuth one`. `UserCustomer_customer_apis` carries \
         `UserCustomer_customer_apis_babelforce`, which the vendor itself describes as `REST API \
         access credentials` and which declares `accessId` and `accessToken` \
         (user-2026-06-25.openapi.yaml:402-415), plus \
         `UserCustomer_customer_apis_stream.token`, described `Push API token` (:417-421). \
         `accessToken` claims `format: uuid` and `The unique Identifier (UUID) of the object`, but \
         that description is boilerplate copied onto both fields and the document contradicts it: \
         the scrubbed example for `accessId` is a real UUID while `accessToken`'s is 32 undashed \
         hex characters (:294). C-415 scrubbed this same block as credential-shaped. The host's \
         redactor holds only values it resolved before the call, so it cannot redact a secret the \
         response itself delivers; C-136 is what lets this come back",
    ),
    // ---- Inexpressible: flux cannot carry a multipart body, so neither can this. ----
    (
        HttpMethod::Post,
        "/api/v2/agents/provision",
        Gap::Inexpressible,
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 established the \
         blocker is flux, not this IR: on the pinned engine line `http.request`'s `body` is a \
         string and `parse`'s `as_type` is a closed analyzer-enforced list with no multipart, so \
         there is no part list, filename or per-part content type to lower to. Needs a flux-side \
         encoder first",
    ),
    (
        HttpMethod::Post,
        "/api/v2/agents/provision/validate",
        Gap::Inexpressible,
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 established the \
         blocker is flux, not this IR: on the pinned engine line `http.request`'s `body` is a \
         string and `parse`'s `as_type` is a closed analyzer-enforced list with no multipart, so \
         there is no part list, filename or per-part content type to lower to. Needs a flux-side \
         encoder first",
    ),
    (
        HttpMethod::Post,
        "/api/v2/outbound/lists/{id}/leads/upload",
        Gap::Inexpressible,
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 established the \
         blocker is flux, not this IR: on the pinned engine line `http.request`'s `body` is a \
         string and `parse`'s `as_type` is a closed analyzer-enforced list with no multipart, so \
         there is no part list, filename or per-part content type to lower to. Needs a flux-side \
         encoder first",
    ),
    (
        HttpMethod::Post,
        "/api/v2/phonebook/bulk",
        Gap::Inexpressible,
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 established the \
         blocker is flux, not this IR: on the pinned engine line `http.request`'s `body` is a \
         string and `parse`'s `as_type` is a closed analyzer-enforced list with no multipart, so \
         there is no part list, filename or per-part content type to lower to. Needs a flux-side \
         encoder first",
    ),
    (
        HttpMethod::Post,
        "/api/v2/prompts",
        Gap::Inexpressible,
        "multipart/form-data request body, which `BodyEncoding` cannot spell; ingest skips it \
         rather than publishing a request that silently sends no body. C-426 established the \
         blocker is flux, not this IR: on the pinned engine line `http.request`'s `body` is a \
         string and `parse`'s `as_type` is a closed analyzer-enforced list with no multipart, so \
         there is no part list, filename or per-part content type to lower to. Needs a flux-side \
         encoder first",
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
/// `babelforce_spec_route.rs`.** Widening to 388 operations meant declaring a blunt
/// `risk = "high"` over every manager write, and three of these nine *are* manager writes with a
/// reviewed `medium` risk. Direction makes all three consequence-bearing, so none may claim the
/// cache-skipping `idempotent` contract; this table pins the reconciled values.
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
        Idempotency::NonIdempotent,
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
        Idempotency::NonIdempotent,
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
        Idempotency::NonIdempotent,
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
    // The document the connector no longer reads, folded back in by name — see [`UNREAD`]. Without
    // this the three endpoints would leave both sides of the comparison at once, which is the one
    // shape of coverage loss this gate exists to make impossible.
    let mut declared: BTreeSet<String> = UNREAD
        .iter()
        .map(|(method, path)| key(*method, path))
        .collect();

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
        .map(|(method, path, _, _)| key(*method, path))
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

    for (method, path, _, reason) in ALLOWED {
        assert!(
            reason.trim().chars().count() >= MIN_REASON,
            "the reason for {} is {} characters, and a reason shorter than {MIN_REASON} tells a \
             reviewer nothing: {reason:?}",
            key(method, path),
            reason.trim().chars().count(),
        );
    }

    // **The accounting, stated as arithmetic rather than as prose, in three categories.**
    // `388 + 5 + 3 = 397`. The categories are counted from the entries' own [`Gap`] rather than by
    // position, so an entry added under the wrong heading fails here instead of being absorbed into
    // a total. Exactly one entry — the receiver — is *outside* the canonical surface rather than
    // missing from it, which is why it is excluded from the sum and why `CANONICAL + 1 == DECLARED`
    // below is the same fact stated from the other end.
    let count = |kind: Gap| ALLOWED.iter().filter(|(_, _, gap, _)| *gap == kind).count();

    assert_eq!(emitted.len(), EMITTED, "the emitted operation count moved");
    assert_eq!(
        count(Gap::OutsideSurface),
        OUTSIDE_SURFACE,
        "the receiver is the only entry outside the canonical surface"
    );
    assert_eq!(
        count(Gap::Inexpressible),
        INEXPRESSIBLE,
        "the inexpressible set is the multipart uploads, and moving it means flux grew a multipart \
         body — re-read C-426 before editing this"
    );
    assert_eq!(
        count(Gap::Withheld),
        WITHHELD,
        "the withheld set is the three `/oauth/*` endpoints plus `GET /api/v2/user/account`; an \
         operation joining or leaving it is a change to what this connector will hand a caller"
    );
    assert_eq!(
        emitted.len() + count(Gap::Inexpressible) + count(Gap::Withheld),
        CANONICAL,
        "{EMITTED} emitted + {INEXPRESSIBLE} inexpressible + {WITHHELD} withheld = the {CANONICAL} \
         manager-sdk covers"
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
// The auth-flow rule
// ---------------------------------------------------------------------------------------------

/// **An OAuth endpoint never becomes a connector operation.**
///
/// Owner-stated 2026-08-01, and **general rather than a babelforce judgement**: an `/oauth/*`
/// endpoint is *how to authenticate*. That is the connector's authentication surface, which
/// `[[auth]]` describes and which the host performs — not something an agent or a caller invokes
/// and reads a result from. The rule is recorded in `AGENTS.md` § Authentication contract.
///
/// v0.9.0 withheld `POST /oauth/token` on the narrower argument that its response body *is* a
/// credential. That was right and under-reasoned, and it left the two siblings emitted, both wrong
/// for the broader reason:
///
/// - `GET /oauth/authorize` is the **PKCE browser redirect** — `response_type`, `redirect_uri`,
///   `code_challenge`, `code_challenge_method`. It is an endpoint a *user-agent* is sent to, not
///   one a program calls and reads a result from.
/// - `POST /oauth/revoke` takes a **`client_secret` as a plain operation argument**, which is
///   auth-flow material travelling as a call parameter — the shape the three-axis auth model exists
///   to keep out of a signature.
///
/// So the `auth` service contributes **zero** operations. Its `[[services]]` and `[[spec]]` entries
/// stay: ingest must keep reading the document so [`DECLARED`] still counts all three and
/// drift-check still watches its `sha256`. A gap this rule creates is still a gap [`ALLOWED`] must
/// explain — which is why all three carry an entry rather than disappearing from the accounting.
#[test]
fn no_oauth_endpoint_becomes_an_operation() {
    let connector = load().connector;

    let oauth: Vec<(&str, &str)> = connector
        .operations
        .iter()
        .filter(|operation| operation.path.starts_with("/oauth"))
        .map(|operation| (operation.id.as_str(), operation.path.as_str()))
        .collect();

    assert!(
        oauth.is_empty(),
        "OAuth endpoints reached the connector as operations: {oauth:?}. An `/oauth/*` endpoint is \
         how a caller authenticates — `[[auth]]` describes it and the host performs it — and it is \
         never an operation. Remove the `auth` selectors from `providers/babelforce.toml` and give \
         each withheld endpoint an `ALLOWED` entry stating the auth-flow rule"
    );
}

// ---------------------------------------------------------------------------------------------
// Exposure — the reason 388 operations is survivable
// ---------------------------------------------------------------------------------------------

/// **Nine tools, not 388.** The exposure tier's whole purpose, asserted as a number.
///
/// C-413 split "callable" from "exposed" precisely so that widening a connector to a vendor's full
/// surface does not spend a model's entire context on a tool list. Without this assertion the
/// mechanism is one absent `expose = false` away from being undone — a selector that loses the key
/// still compiles, still emits every operation, and turns the catalogue into 388 LLM tools.
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
        "the exposed set is not the curated nine. 388 operations are callable and nine are tools; \
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

    // Order, too: the nine publish first and in the file's block order, which is the order
    // `connectors/babelforce-manager.flux` and `crates/catalog/src/generated/babelforce.rs` render
    // them in and therefore what a reviewer diffs.
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
/// rule is that each either gets one through the overlay or stays unexposed — and since 382 of the
/// 388 are unexposed, that is very nearly free. This is the assertion that keeps it free rather
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
