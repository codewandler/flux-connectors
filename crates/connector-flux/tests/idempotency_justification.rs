//! **A `POST` or `PATCH` may declare the idempotency it actually has — but only by saying why.**
//!
//! [`check_write_metadata`](../src/op.rs) refused `idempotency = "idempotent"` on `POST` and `PATCH`
//! by method, unconditionally. The refusal is right for the general case and stays: RFC 9110 §9.2.2
//! makes neither method idempotent, and a `retry` wrapped around a write that is not is unsound. But
//! it was also right for `cloudflare-cache-purge` and `launchdarkly-flag-toggle`, which are
//! idempotent by their vendors' own behaviour — so each shipped `non_idempotent` with a comment
//! saying the opposite, and **the comment is not what a host reads** (C-186).
//!
//! `idempotent_because` is the escape, and the shape of it is the whole point:
//!
//! - the method-based refusal is **unchanged** where the author says nothing, so the careless case —
//!   a `POST` that claims to be idempotent because the author copied a `GET` — still fails the build;
//! - the deliberate case becomes expressible, and expressible **only with its reason attached**,
//!   which reaches `web/public/catalog.json` and a reviewer rather than dying in a TOML comment;
//! - the reason is refused where nothing was refusing the claim (`GET`, `PUT`, `DELETE`) and where
//!   the claim is not being made, so the field never becomes decoration.
//!
//! Every case here is built by **loading a provider TOML**, not by assembling an [`Operation`] in
//! memory: the loader's accepted input is the surface a downstream connector author actually writes,
//! and it is the surface this story widens.

use connector_flux::emit_operation;
use connector_spec::provider;

/// The smallest provider file that reaches the `POST` refusal: one write, whatever idempotency and
/// justification the caller wants to try.
fn provider_toml(method: &str, idempotency: &str, justification: Option<&str>) -> String {
    let because = justification
        .map(|reason| format!("idempotent_because = {reason:?}\n"))
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

/// **The change this story is for.** A `POST` that states why it is idempotent emits, and the
/// declaration a host reads says `idempotent` — not the safe-but-false `non_idempotent` that
/// `cloudflare-cache-purge` shipped for want of a way to say this.
#[test]
fn a_post_may_declare_idempotent_when_it_declares_why() {
    let flux = emit(&provider_toml(
        "POST",
        "idempotent",
        Some("purging an already-purged cache is a no-op"),
    ))
    .expect("a POST that says why it is idempotent must emit");

    assert!(
        flux.contains(r#"idempotency "idempotent""#),
        "the emitted op must carry the idempotency the author declared — this field is what flux's \
         retry and approval logic reads, and it was the one carrying a value nobody authored:\n{flux}"
    );
}

/// The same for `PATCH`, which is the half `launchdarkly-flag-toggle` hit — a `replace` onto a
/// boolean, landing in the same state however many times it runs.
#[test]
fn a_patch_may_declare_idempotent_when_it_declares_why() {
    let flux = emit(&provider_toml(
        "PATCH",
        "idempotent",
        Some("replacing one boolean with the same boolean lands in one state"),
    ))
    .expect("a PATCH that says why it is idempotent must emit");

    assert!(flux.contains(r#"idempotency "idempotent""#), "{flux}");
}

/// **The guard that must not be weakened.** No justification, no claim: this is exactly the
/// behaviour every `POST` in the catalogue has today, and it is what stops an author who copied a
/// read's metadata onto a write from shipping "safe to retry" by accident.
#[test]
fn a_post_declaring_idempotent_without_a_reason_is_still_refused() {
    let refusal = emit(&provider_toml("POST", "idempotent", None))
        .expect_err("a POST claiming idempotency with no reason must still be refused");

    assert!(
        refusal.contains("acme-cache-purge") && refusal.contains("idempotent"),
        "the refusal must name the operation and the claim it is refusing: {refusal}"
    );
}

/// A `PATCH` likewise. Stated separately because the two methods are refused by one `matches!` and a
/// change that dropped either arm would leave the other test green.
#[test]
fn a_patch_declaring_idempotent_without_a_reason_is_still_refused() {
    let refusal = emit(&provider_toml("PATCH", "idempotent", None))
        .expect_err("a PATCH claiming idempotency with no reason must still be refused");

    assert!(refusal.contains("acme-cache-purge"), "{refusal}");
}

/// **An escape anyone can take without saying anything is not an escape, it is a removal.** A blank
/// reason, and a reason too short to be one, are refused — the floor is the length of the shortest
/// honest justification anyone in this repository has actually written.
#[test]
fn a_reason_that_says_nothing_does_not_unlock_the_claim() {
    for shrug in ["", "   ", "yes", "idempotent", "see above"] {
        let refusal = emit(&provider_toml("POST", "idempotent", Some(shrug)))
            .expect_err("a reason that states nothing must not unlock the claim: {shrug:?}");
        assert!(
            refusal.contains("idempotent_because"),
            "the refusal must name the field the author needs to fix, given {shrug:?}: {refusal}"
        );
    }
}

/// The field is refused where nothing was refusing the claim. `GET`, `PUT` and `DELETE` may declare
/// `idempotent` freely — a justification there addresses no refusal, and a field that may be written
/// anywhere becomes decoration nobody reads.
#[test]
fn a_reason_is_refused_where_the_method_never_needed_one() {
    for method in ["GET", "PUT", "DELETE"] {
        let refusal = emit(&provider_toml(
            method,
            "idempotent",
            Some("purging an already-purged cache is a no-op"),
        ))
        .expect_err("`idempotent_because` on a method that was never refused must be refused");
        assert!(
            refusal.contains("idempotent_because"),
            "given {method}: {refusal}"
        );
    }
}

/// A reason attached to a declaration that is not `idempotent` is prose contradicting its own
/// field — the exact defect C-186 exists to remove, arriving from the other direction.
#[test]
fn a_reason_is_refused_when_the_operation_does_not_claim_idempotency() {
    let refusal = emit(&provider_toml(
        "POST",
        "non_idempotent",
        Some("purging an already-purged cache is a no-op"),
    ))
    .expect_err("a justification for a claim the operation does not make must be refused");

    assert!(
        refusal.contains("idempotent_because"),
        "the refusal must name the field: {refusal}"
    );
}
