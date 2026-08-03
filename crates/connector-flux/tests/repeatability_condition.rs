//! **A write that is genuinely safe to repeat may say so — by stating the condition that makes it
//! safe.**
//!
//! `check_write_metadata` refuses `idempotency = "idempotent"` on `POST` and `PATCH` by method, and
//! C-186 leaves that refusal exactly as strict as it found it. What the story was really about is
//! that three connectors — Cloudflare's cache purge, LaunchDarkly's flag toggle, Miro's sticky-note
//! update — are safely repeatable by their vendors' behaviour and had no way to say it, so each
//! shipped `non_idempotent` beside a comment saying the opposite. **A comment is not what a host
//! reads.**
//!
//! The value that says it is `Idempotency::Conditional`, which is flux's own escape hatch for
//! precisely this (`flux_spec::coherence`, I3: *"`Conditional` — not a loosened rule — is the escape
//! hatch for 'safely repeatable'"*). It was permitted here all along, on every method, **with
//! nothing asked of it** — six operations used it with the condition recorded in no field and no
//! artifact. So C-186 is a tightening, not a loosening:
//!
//! - a mutating `conditional` must **state its condition**, because flux's wording is "safe to
//!   repeat under *stated* conditions" and nothing was making anyone state them;
//! - the condition is refused where it means nothing — on a method that changes nothing, and beside
//!   an `idempotency` that is not `conditional`;
//! - `idempotent` on a `POST`/`PATCH` stays refused **unconditionally**, because that value licenses
//!   flux's op cache to serve a stored result *instead of executing*, which is a stronger claim than
//!   "repeating is safe" and not one a cache purge can make.
//!
//! # Two routes, deliberately tested twice
//!
//! Most cases here are built by **loading a provider TOML**, because that is the surface a connector
//! author actually writes. But the loader and the emitter each enforce these rules, and in the first
//! landing of this story every emitter arm was reached only through the loader — so replacing an
//! emitter refusal with `if false` left the whole workspace green, and widening the loader's method
//! set left it green too. Each layer's coverage came entirely from the other.
//!
//! [`the_emitter_refuses_an_in_memory_ir`] closes that: it builds `Operation` values directly and
//! never calls the loader, which is the route both files claim the emitter arm exists for.

use connector_flux::emit_operation;
use connector_spec::{
    provider, Connector, HttpMethod, Idempotency, Operation, ParamSet, Provenance, Quirks, Risk,
    DEFAULT_SERVICE,
};

/// The smallest provider file that reaches these guards: one operation, whatever method,
/// idempotency and condition the caller wants to try.
fn provider_toml(method: &str, idempotency: &str, condition: Option<&str>) -> String {
    let because = condition
        .map(|reason| format!("repeatable_because = {reason:?}\n"))
        .unwrap_or_default();
    format!(
        r#"
id = "acme"
vendor = "Acme"
description = "A connector that exists to exercise one guard"
base_url = "https://api.acme.test"
api_version = "v1"

[[operations]]
id = "acme-cache-purge"
method = "{method}"
path = "/cache/purge"
description = "Empty the cache. Emptying an empty cache empties it again"
risk = "high"
idempotency = "{idempotency}"
{because}"#
    )
}

/// Load and emit, returning the formatted Flux or the first refusal as a string.
fn emit(toml: &str) -> Result<String, String> {
    let loaded = provider::load("acme", toml).map_err(|error| error.to_string())?;
    let connector = loaded.connector;
    let operation = connector
        .operations
        .first()
        .expect("the fixture declares one operation");
    emit_operation(&connector, operation).map_err(|error| error.to_string())
}

/// **The change this story is for.** A `POST` that states the condition making it replay-safe emits,
/// and the declaration a host reads says `conditional` — not the safe-but-false `non_idempotent`
/// that `cloudflare-cache-purge` shipped for want of a way to say this.
#[test]
fn a_post_may_declare_conditional_when_it_states_the_condition() {
    let flux = emit(&provider_toml(
        "POST",
        "conditional",
        Some("purging an already-purged cache is a no-op"),
    ))
    .expect("a POST that states its repeat condition must emit");

    assert!(
        flux.contains(r#"idempotency "conditional""#),
        "the emitted op must carry the idempotency the author declared — this field is what flux's \
         retry logic reads, and it was the one carrying a value nobody authored:\n{flux}"
    );
}

/// The same for `PATCH`, which is the half `launchdarkly-flag-toggle` hit.
#[test]
fn a_patch_may_declare_conditional_when_it_states_the_condition() {
    let flux = emit(&provider_toml(
        "PATCH",
        "conditional",
        Some("replacing one boolean with the same boolean lands in one state"),
    ))
    .expect("a PATCH that states its repeat condition must emit");

    assert!(flux.contains(r#"idempotency "conditional""#), "{flux}");
}

/// **The tightening.** `conditional` on a write with no stated condition used to emit — this is the
/// rule C-186 adds, and the reason six shipped operations were publishing a claim with no evidence
/// anywhere behind it.
#[test]
fn a_conditional_write_that_states_no_condition_is_refused() {
    for method in ["POST", "PUT", "PATCH", "DELETE"] {
        let refusal = emit(&provider_toml(method, "conditional", None))
            .expect_err("a mutating `conditional` with no stated condition must be refused");
        assert!(
            refusal.contains("repeatable_because") && refusal.contains("acme-cache-purge"),
            "given {method}, the refusal must name the operation and the field: {refusal}"
        );
    }
}

/// **The guard that must not be weakened, and was not.** `idempotent` on a `POST` is refused with or
/// without a condition — the value licenses flux's op cache to skip execution entirely, which is not
/// what any of these three vendors offer.
#[test]
fn a_post_declaring_idempotent_is_still_refused_with_or_without_a_condition() {
    for condition in [None, Some("purging an already-purged cache is a no-op")] {
        let refusal = emit(&provider_toml("POST", "idempotent", condition))
            .expect_err("a POST claiming `idempotent` must be refused however it is dressed");
        assert!(
            refusal.contains("acme-cache-purge"),
            "the refusal must name the operation: {refusal}"
        );
    }
}

/// A `PATCH` likewise. Stated separately because the two methods are refused by one `matches!` and a
/// change that dropped either arm would leave the other test green.
#[test]
fn a_patch_declaring_idempotent_is_still_refused() {
    let refusal = emit(&provider_toml("PATCH", "idempotent", None))
        .expect_err("a PATCH claiming `idempotent` must be refused");
    assert!(refusal.contains("acme-cache-purge"), "{refusal}");
}

/// **An escape anyone can take without saying anything is not an escape, it is a removal.** A blank
/// condition, and one too short to be one, are refused — the floor is the length of the shortest
/// honest condition anyone in this repository has actually written.
#[test]
fn a_condition_that_says_nothing_does_not_unlock_the_claim() {
    for shrug in ["", "   ", "yes", "idempotent", "see above"] {
        let refusal = emit(&provider_toml("POST", "conditional", Some(shrug)))
            .expect_err("a condition that states nothing must not unlock the claim");
        assert!(
            refusal.contains("repeatable_because"),
            "the refusal must name the field the author needs to fix, given {shrug:?}: {refusal}"
        );
    }
}

/// **`trim` is load-bearing, not cosmetic.** More than `MIN_REPEATABILITY_CONDITION` spaces clears
/// the character floor and states nothing whatsoever; without the trim it would unlock the claim.
///
/// Split from the case above because that one would still pass with the trim deleted — `""` and
/// `"yes"` are shorter than the floor either way — so it cannot see this defect.
#[test]
fn a_condition_of_pure_whitespace_does_not_clear_the_floor() {
    let whitespace = " ".repeat(connector_spec::MIN_REPEATABILITY_CONDITION + 6);
    let refusal = emit(&provider_toml("POST", "conditional", Some(&whitespace))).expect_err(
        "whitespace long enough to clear the character floor must still be refused — it states \
         nothing, and a floor measured before trimming is a floor on keystrokes rather than on \
         content",
    );
    assert!(refusal.contains("repeatable_because"), "{refusal}");
}

/// The field is refused where nothing about the method can repeat harmfully.
#[test]
fn a_condition_is_refused_on_a_method_that_changes_nothing() {
    for method in ["GET", "HEAD", "OPTIONS"] {
        let refusal = emit(&provider_toml(
            method,
            "idempotent",
            Some("purging an already-purged cache is a no-op"),
        ))
        .expect_err("`repeatable_because` on a method that changes nothing must be refused");
        assert!(
            refusal.contains("repeatable_because"),
            "given {method}: {refusal}"
        );
    }
}

/// A condition attached to a declaration that is not `conditional` is prose contradicting its own
/// field — the exact defect C-186 exists to remove, arriving from the other direction.
#[test]
fn a_condition_is_refused_when_the_operation_does_not_claim_conditional() {
    for idempotency in ["non_idempotent", "idempotent"] {
        let refusal = emit(&provider_toml(
            "PUT",
            idempotency,
            Some("purging an already-purged cache is a no-op"),
        ))
        .expect_err("a condition for a claim the operation does not make must be refused");
        assert!(
            refusal.contains("repeatable_because"),
            "given {idempotency}: {refusal}"
        );
    }
}

// ---------------------------------------------------------------------------
// The emitter, reached without the loader
// ---------------------------------------------------------------------------

/// One in-memory operation. **Nothing here goes through `provider::load`** — that is the whole point
/// of this section.
fn operation(method: HttpMethod, idempotency: Idempotency, condition: Option<&str>) -> Operation {
    Operation {
        id: "acme-cache-purge".to_owned(),
        service: DEFAULT_SERVICE.to_owned(),
        method,
        path: "/cache/purge".to_owned(),
        description: "Empty the cache.".to_owned(),
        risk: Risk::High,
        idempotency,
        semantic_effects: Vec::new(),
        repeatable_because: condition.map(str::to_owned),
        expose: true,
        auth: None,
        params: ParamSet::default(),
        response_schema: None,
        credential_response: Vec::new(),
        produces_credential: None,
        quirks: Quirks::default(),
    }
}

/// A connector holding exactly `operation`.
fn connector(operation: Operation) -> Connector {
    Connector {
        id: "acme".to_owned(),
        authority: None,
        runtime: connector_spec::Runtime::Http,
        api_version: None,
        services: Vec::new(),
        vendor: String::new(),
        base_url: "https://api.acme.test".to_owned(),
        description: String::new(),
        auth: Vec::new(),
        default_auth: Vec::new(),
        operations: vec![operation],
        events: Vec::new(),
        channels: Vec::new(),
        config: Vec::new(),
        verify: None,
        graphs: Vec::new(),
        provenance: Provenance::default(),
    }
}

/// **Each layer is pinned on its own.** `connector-spec` refuses these in a provider file and
/// `connector-flux` refuses them again on the IR, and both files say the emitter arm exists for the
/// in-memory route the loader never sees. Until this test, that claim was untested in both
/// directions: replacing an emitter arm's condition with `if false` left the entire workspace green,
/// because every case reaching it came in through the loader, which had already refused. Widening
/// the loader's own method set was invisible for the mirror-image reason.
///
/// A defence whose only coverage comes from a different defence is not a defence, it is a comment.
#[test]
fn the_emitter_refuses_an_in_memory_ir() {
    let cases: [(&str, Operation); 4] = [
        (
            "a condition on a method that changes nothing",
            operation(
                HttpMethod::Get,
                Idempotency::Idempotent,
                Some("reading a list twice reads the same list"),
            ),
        ),
        (
            "a condition beside an idempotency that is not conditional",
            operation(
                HttpMethod::Put,
                Idempotency::NonIdempotent,
                Some("purging an already-purged cache is a no-op"),
            ),
        ),
        (
            "a mutating conditional stating no condition",
            operation(HttpMethod::Post, Idempotency::Conditional, None),
        ),
        (
            "a mutating conditional whose condition says nothing",
            operation(HttpMethod::Post, Idempotency::Conditional, Some("yes")),
        ),
    ];

    for (what, operation) in cases {
        let connector = connector(operation);
        let attempt = emit_operation(&connector, &connector.operations[0]);
        assert!(
            attempt.is_err(),
            "the emitter accepted {what}. The loader refuses this too, so the workspace stays green \
             while this arm does nothing — which is exactly how it went untested the first time"
        );
    }
}

/// The positive half, also without the loader: a well-formed in-memory `conditional` write emits.
///
/// Without this, the test above could pass against an emitter that refused *everything*.
#[test]
fn the_emitter_accepts_a_well_formed_in_memory_conditional_write() {
    let connector = connector(operation(
        HttpMethod::Post,
        Idempotency::Conditional,
        Some("purging an already-purged cache is a no-op"),
    ));
    let flux = emit_operation(&connector, &connector.operations[0])
        .expect("a well-formed conditional write emits");
    assert!(flux.contains(r#"idempotency "conditional""#), "{flux}");
}
