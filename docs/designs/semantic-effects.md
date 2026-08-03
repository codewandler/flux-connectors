# Semantic effects are a second, policy-bearing axis

**Story:** C-155
**Decision:** accepted 2026-08-03

## Decision

Connector operations keep the host-resource `effects` emitted in Flux and gain a distinct
`semantic_effects` declaration in the connector IR. The former says which host resource execution
touches; the latter says what the operation means to Flux policy. Generated HTTP operations remain
`effects ["network"]` exactly. Semantic tags travel beside that declaration through the installable
manifest, embedded catalogue, public catalogue and `Tool::semantic_effects()`.

The provider vocabulary is the non-deprecated `flux_spec::FlowEffect` vocabulary shipped with Flux
1.3: `pure`, `read`, `model`, `network`, `write_file`, `write_db`, `send_external`, `delete`, `money`
and `human_visible`. `calendar` is deprecated upstream and is not accepted here. The compiler crates
do not depend on `flux-spec`, so `connector-spec` owns the serializable enum and `connector-pack`
proves every emitted tag round-trips through Flux's `FlowEffect::from_tag` before exposing it.

## Refusal boundary

Unknown and duplicate values are load errors. `pure` is refused because an `Operation` is an HTTP
call. `money` and `delete` require `risk = "destructive"`; every other consequential tag requires a
non-low risk, and no consequential operation may claim `idempotent`. These are the same floors Flux
applies after `Tool::semantic_effects()` reaches its registry.

The compiler cannot infer that an operation moves money from `POST`, `destructive`, or prose: each
also describes non-financial actions. The shipped money census is therefore a hard catalogue test,
not a blanket rule that mislabels every irreversible POST. Provider work that adds a money-moving
operation adds that provider's evidence beside its declaration.

## Stripe worked example

- `stripe-payment-intent-capture` moves money and declares `money`; Flux's I2 floor raises its risk
  from `high` to `destructive`.
- `stripe-charge-refund-create` moves money and declares `money`. It does not declare `delete`: a
  refund is irreversible, but no entity is deleted.
- `stripe-payment-intent-cancel` releases an authorization and moves no money, so it declares no
  semantic effect and remains `high`.

## Wire compatibility

Provider TOML and canonical IR omit an empty list. The existing manifest `operations` list remains
unchanged; an additive `operation_semantic_effects` map names every operation and always carries an
array. `catalog::Operation` gains an additive field under its existing `#[non_exhaustive]` contract,
and every public `catalog.json` operation gains an always-present `semantic_effects` array.
