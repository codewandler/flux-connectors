//! Whole-catalogue coherence at the seam where connector truth becomes a Flux [`ToolSpec`].
//!
//! Direction is authored in provider TOML and carried by the embedded catalogue. These tests never
//! reconstruct it from the request method: doing so would make Babelforce's GET-shaped dialer flush
//! look like a read again.

use catalog::{Operation, OperationDirection};
use flux_spec::{coherence, Effect, Idempotency, ToolSpec};

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

fn semantic_effects(operation: &Operation) -> Vec<String> {
    operation
        .semantic_effects
        .iter()
        .map(|effect| (*effect).to_owned())
        .collect()
}

fn projected(operation: &Operation) -> ToolSpec {
    connector_pack::project(operation)
        .unwrap_or_else(|error| panic!("`{}` projects to a ToolSpec: {error}", operation.id))
}

#[test]
fn every_tool_spec_copies_the_authored_direction_before_network() {
    for operation in shipped() {
        let expected = match operation.direction {
            OperationDirection::Read => vec![Effect::Read, Effect::Network],
            OperationDirection::Write => vec![Effect::Write, Effect::Network],
        };
        assert_eq!(
            projected(operation).effects,
            expected,
            "`{}` did not preserve its authored {:?} direction in canonical effect order",
            operation.id,
            operation.direction,
        );
    }
}

#[test]
fn every_shipped_operation_satisfies_flux_metadata_coherence() {
    let mut violations = Vec::new();
    for operation in shipped() {
        for violation in
            coherence::metadata_violations(&projected(operation), &semantic_effects(operation))
        {
            violations.push(format!("{}: {violation}", operation.id));
        }
    }
    assert!(
        violations.is_empty(),
        "connector metadata contradicts Flux's canonical coherence rules:\n{}",
        violations.join("\n")
    );
}

#[test]
fn no_authored_write_claims_flux_may_skip_its_execution() {
    let offenders: Vec<&str> = shipped()
        .into_iter()
        .filter(|operation| operation.direction == OperationDirection::Write)
        .filter(|operation| projected(operation).idempotency == Idempotency::Idempotent)
        .map(|operation| operation.id)
        .collect();
    assert!(
        offenders.is_empty(),
        "writes may not claim `idempotent`, which licenses Flux to reuse a cached result instead \
         of executing them: {offenders:?}"
    );
}

#[test]
fn every_conditional_operation_is_an_authored_write() {
    let conditional: Vec<&Operation> = shipped()
        .into_iter()
        .filter(|operation| projected(operation).idempotency == Idempotency::Conditional)
        .collect();
    assert!(
        !conditional.is_empty(),
        "no shipped operation declares `conditional`, so this assertion proves nothing"
    );
    for operation in conditional {
        assert_eq!(
            operation.direction,
            OperationDirection::Write,
            "`{}` weakens a read with conditional idempotency",
            operation.id,
        );
    }
}
