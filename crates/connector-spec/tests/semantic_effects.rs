//! Semantic effects are a closed, fail-closed policy vocabulary, separate from host effects.

use connector_spec::{provider, Risk, SemanticEffect};

fn operation(effect_line: &str, risk: &str, idempotency: &str) -> String {
    format!(
        r#"
id = "acme"
base_url = "https://api.acme.test"

[[operations]]
id = "acme-action"
method = "POST"
direction = "write"
path = "/action"
description = "Perform the action."
risk = "{risk}"
idempotency = "{idempotency}"
repeatable_because = "the caller supplies one stable key and the vendor replays the first result"
{effect_line}
"#
    )
}

#[test]
fn unknown_semantic_effects_are_refused_at_load() {
    let error = provider::load(
        "acme",
        &operation(
            r#"semantic_effects = ["telepathy"]"#,
            "destructive",
            "conditional",
        ),
    )
    .expect_err("an unknown Flux tag must not load")
    .to_string();

    assert!(error.contains("unknown variant `telepathy`"), "{error}");
}

#[test]
fn duplicate_semantic_effects_are_refused_instead_of_deduped() {
    let error = provider::load(
        "acme",
        &operation(
            r#"semantic_effects = ["money", "money"]"#,
            "destructive",
            "conditional",
        ),
    )
    .expect_err("a duplicate claim must not be silently normalised")
    .to_string();

    assert!(
        error.contains("semantic effect \"money\" more than once"),
        "{error}"
    );
}

#[test]
fn money_and_delete_require_the_destructive_risk_floor() {
    let error = provider::load(
        "acme",
        &operation(r#"semantic_effects = ["money"]"#, "high", "conditional"),
    )
    .expect_err("money at high risk would clear Flux's destructive floor")
    .to_string();

    assert!(
        error.contains("semantic effect \"money\" but risk \"high\""),
        "{error}"
    );
}

#[test]
fn consequential_effects_cannot_claim_idempotent() {
    let source = operation(
        r#"semantic_effects = ["send_external"]"#,
        "high",
        "idempotent",
    )
    .replace(
        "repeatable_because = \"the caller supplies one stable key and the vendor replays the first result\"\n",
        "",
    );
    let error = provider::load("acme", &source)
        .expect_err("Flux may not skip a consequence in favour of a cached result")
        .to_string();

    assert!(error.contains("consequential semantic effect"), "{error}");
}

#[test]
fn pure_is_not_a_truthful_effect_for_an_http_operation() {
    let source = operation(r#"semantic_effects = ["pure"]"#, "low", "idempotent").replace(
        "repeatable_because = \"the caller supplies one stable key and the vendor replays the first result\"\n",
        "",
    );
    let error = provider::load("acme", &source)
        .expect_err("an external HTTP call is not pure")
        .to_string();

    assert!(error.contains("semantic effect `pure`"), "{error}");
}

#[test]
fn semantic_effects_have_one_canonical_order() {
    let loaded = provider::load(
        "acme",
        &operation(
            r#"semantic_effects = ["money", "delete"]"#,
            "destructive",
            "conditional",
        ),
    )
    .expect("two coherent effects load");

    assert_eq!(
        loaded.connector.operations[0].semantic_effects,
        vec![SemanticEffect::Delete, SemanticEffect::Money]
    );
    assert_eq!(loaded.connector.operations[0].risk, Risk::Destructive);
}
