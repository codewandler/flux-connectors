//! **flux owns the metadata vocabulary, so flux gets to say whether we are speaking it.**
//!
//! `flux_spec::coherence` ships three invariants over a `ToolSpec`, and this crate is the one seam
//! where a shipped operation becomes one. I3 — *the repeatability floor* — is the one C-186 is
//! about: a consequence-bearing spec must not declare `Idempotency::Idempotent`, and flux names
//! `Conditional` as the escape hatch for a mutation that is genuinely safe to repeat.
//!
//! # What this covers, and the two gaps it deliberately leaves open
//!
//! Measured on 2026-08-01 over all 299 shipped operations: **I3 fires on 204 of them**, and only 12
//! of those are mutations. Both remainders are real findings, and neither is C-186's to fix.
//!
//! **Gap 1 — 192 reads trip a rule aimed at writes.** `is_consequence_bearing` classifies a spec by
//! its effect set, and `[Network]` without `Effect::Read` is consequence-bearing. Every operation
//! this repository emits declares exactly `effects ["network"]`, a `GET` included, so every read
//! looks like a write to flux. Fixing it means emitting `Effect::Read` alongside `Effect::Network`
//! for non-mutating methods — which moves every artifact in the catalogue, and is its own story.
//!
//! **Gap 2 — nine shipped `PUT`s claim `Idempotent`, and this repository permits them on purpose.**
//! `check_write_metadata` allows `idempotent` on `PUT` and `DELETE` because RFC 9110 §9.2.2 makes
//! those methods idempotent. flux's I3 does not consider the method at all: it refuses `Idempotent`
//! on anything consequence-bearing, because the value licenses the op cache to serve a stored result
//! *instead of executing*. Replaying a `PUT` is safe; **skipping** one is not, so the two rules are
//! in genuine conflict rather than one being a sloppy version of the other. The nine are
//! `babelforce-agent-status-update`, `babelforce-call-session-set`, `babelforce-session-update`,
//! `contentful-entry-publish`, `freshdesk-ticket-update`, `mailchimp-audience-member-upsert`,
//! `pagerduty-incident-acknowledge`, `pagerduty-incident-resolve` and `trello-card-archive`.
//! Resolving that is a decision about whose vocabulary wins across eight providers.
//!
//! So the assertion below is quantified over `POST` and `PATCH`: the boundary is **principled rather
//! than a list of ids** — it is exactly where this repository and flux already agree — so a new
//! connector cannot falsify it merely by existing, and it is precisely the ground C-186 stands on.
//! Widening it to every mutation is the follow-up story, and it is red today by nine.

use catalog::Operation;
use flux_spec::coherence;

/// Every operation the catalogue ships.
fn shipped() -> Vec<&'static Operation> {
    let operations: Vec<&Operation> = catalog::providers()
        .iter()
        .flat_map(|provider| provider.operations.iter())
        .collect();
    assert!(
        !operations.is_empty(),
        "the embedded catalogue is empty, so every assertion below would pass vacuously"
    );
    operations
}

/// Whether the operation is sent with `method`.
///
/// Read off the emitted Flux rather than re-derived: `catalog::Operation` carries no method, and the
/// `op` body's `http.request(… method: "POST" …)` is the shipped truth about what it sends.
fn sent_with(operation: &Operation, method: &str) -> bool {
    operation.flux.contains(&format!("method: {method:?}"))
}

/// Whether the operation's method changes state the vendor owns.
fn mutates(operation: &Operation) -> bool {
    ["POST", "PUT", "PATCH", "DELETE"]
        .iter()
        .any(|method| sent_with(operation, method))
}

/// The methods RFC 9110 §9.2.2 does **not** make idempotent — where this repository's rule and
/// flux's I3 agree exactly, and the ground C-186 stands on.
fn not_idempotent_by_method(operation: &Operation) -> bool {
    sent_with(operation, "POST") || sent_with(operation, "PATCH")
}

/// **No `POST` or `PATCH` may claim `Idempotent`** — flux's I3, asserted against flux's own checker
/// rather than against this repository's reading of it.
///
/// This is the assertion C-186 was reworked around. The story's first landing gave three of these
/// operations `Idempotency::Idempotent` behind a justification field, on the reasoning that they are
/// idempotent by their vendors' behaviour. They are — but `Idempotent` is the value that licenses
/// flux's op cache to serve a stored result *instead of executing*, and "safe to repeat" and "safe
/// to not run at all" are different claims. flux reserves the first for `Conditional` and says so.
///
/// See the module docs for the two populations this deliberately does not quantify over, and why.
#[test]
fn no_post_or_patch_violates_the_repeatability_floor() {
    let mut violations = Vec::new();

    for operation in shipped() {
        if !not_idempotent_by_method(operation) {
            continue;
        }
        let spec = connector_pack::project(operation)
            .unwrap_or_else(|error| panic!("`{}` projects to a ToolSpec: {error}", operation.id));
        // No semantic-effect tags: the pack advertises none, so the effect-set channel is the only
        // one in play — which is the channel I3 reads for a write.
        for violation in coherence::metadata_violations(&spec, &[]) {
            if violation.starts_with("I3") {
                violations.push(format!("{}: {violation}", operation.id));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "these operations are sent with a method RFC 9110 §9.2.2 does not make idempotent, and \
         claim `Idempotency::Idempotent` anyway — which flux's own coherence module refuses. Use \
         `conditional` and state the condition in `repeatable_because` (C-186):\n{}",
        violations.join("\n")
    );
}

/// **The nine-`PUT` divergence, pinned as a number so it cannot grow quietly.**
///
/// Gap 2 in the module docs is a real conflict between this repository's RFC-based rule and flux's
/// I3, and resolving it is a separate story. What must not happen meanwhile is the number drifting
/// upward unnoticed while everyone believes it is "the known nine".
///
/// This is deliberately a **two-way** assertion. Fewer is as interesting as more: it means somebody
/// resolved part of the conflict, and the follow-up story's scope moved.
#[test]
fn the_known_rfc_idempotent_divergence_from_flux_has_not_grown() {
    let diverging: Vec<&str> = shipped()
        .into_iter()
        .filter(|operation| mutates(operation) && !not_idempotent_by_method(operation))
        .filter(|operation| {
            connector_pack::project(operation).is_ok_and(|spec| {
                coherence::metadata_violations(&spec, &[])
                    .iter()
                    .any(|violation| violation.starts_with("I3"))
            })
        })
        .map(|operation| operation.id)
        .collect();

    assert_eq!(
        diverging.len(),
        9,
        "the count of `PUT`/`DELETE` operations claiming `Idempotent` against flux's I3 moved. \
         This repository permits them (RFC 9110 §9.2.2) and flux refuses them (the op cache may \
         skip execution), and the conflict is filed rather than resolved — but the population is \
         pinned so it cannot grow while nobody is looking. Found: {diverging:#?}"
    );
}

/// **A `Conditional` claim is worth nothing unless the condition is stated**, and flux's wording is
/// precise about it: *"safe to repeat under **stated** conditions"*.
///
/// The loader enforces this on the provider file; this asserts it survives all the way to what a
/// host is handed, over the real catalogue rather than a fixture. Six operations declared
/// `Conditional` before C-186 with the condition recorded nowhere at all — three of them Stripe
/// money movements.
#[test]
fn every_conditional_operation_is_a_mutation_whose_condition_was_stated() {
    let conditional: Vec<&Operation> = shipped()
        .into_iter()
        .filter(|operation| {
            connector_pack::project(operation)
                .is_ok_and(|spec| spec.idempotency == flux_spec::Idempotency::Conditional)
        })
        .collect();

    assert!(
        !conditional.is_empty(),
        "no shipped operation declares `conditional`, so this assertion proves nothing — if the \
         catalogue really carries none, delete this test rather than leaving it green and empty"
    );

    for operation in &conditional {
        assert!(
            mutates(operation),
            "`{}` declares `conditional` but changes no state; on a read the claim is weaker than \
             the method already gives and says nothing",
            operation.id
        );
    }
}
