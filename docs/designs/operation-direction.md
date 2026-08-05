# Explicit connector operation direction

## Decision

Every connector operation carries one required closed vendor-state direction: `read` or `write`.
It is connector truth, independent of transport. HTTP method remains request syntax and never
supplies a default, fallback, validator, or correction for direction.

Inline `[[operations]]` state `direction` directly. Spec-backed operations receive it from
`[patch.directions.<service>]`, keyed by stable vendor `operationId`, or from an exact
`[[patch.operations]]` block. If both state a value, they must agree. Bulk selectors may still group
transport, risk, idempotency and exposure declarations, but they cannot state direction; changing a
method, path, name or description therefore cannot rematch an operation into another direction.
Composition refuses a selected operation when neither identity-keyed source states direction and
refuses orphaned map keys after an upstream identity rename.

The existing catalogue is migrated conservatively. Every published identity has an explicit
reviewed value. Read-shaped candidates without an individual semantic review retain `write` until a
later provider-specific review proves otherwise; their HTTP method supplies no evidence and changing
it cannot alter the stored value. `babelforce-flush-dialer` is the adversarial case: its GET transport
remains GET, while its declared direction is `write` because the vendor says it flushes tasks.

## Projection

Direction travels unchanged through all operation metadata surfaces:

- generated Flux emits `[Read, Network]` for reads and `[Write, Network]` for writes, in that order;
- manifests and public catalogues serialize the same `read|write` value;
- the dependency-free embedded catalogue exposes the same closed two-value projection;
- `connector-pack` copies the generated host effects into `ToolSpec` and derives concrete intents
  from direction: `NetworkFetch`/`ReadTarget` or `NetworkConnect`/`WriteTarget` over the exact same
  resolved destination used by the permission subject and request.

The operation tool reports `Gather` for an authored read and `Capture` for an authored write. That
hint deliberately repeats no risk, idempotency, semantic-effect or argument classifier. Flux's
public canonical gather admission still applies those contracts, concrete invocation intents,
authorization and runtime state. A whole-catalogue connector test proves the direction projection,
and a host-graph compatibility test executes Flux's public canonical seam on the method-defying read
and write counterexamples. Connector code does not classify arguments or permission subjects as
concurrency keys.

## Coherence

The loader and whole-catalogue tests enforce these invariants:

- omission and unknown values refuse before emission;
- read/write direction agrees with host effects, concrete intents, staging, and Flux's consequence
  predicate;
- every write is consequence-bearing and visible through `Effect::Write` even when unexposed;
- only a low-risk, idempotent, semantically inert read with non-mutating concrete intents can pass
  Flux's canonical gather contract;
- changing an operation's HTTP method alone cannot change any of those answers;
- no declared write keeps `Idempotent`; safe replay of a mutation uses `Conditional` with an
  authored condition, otherwise `NonIdempotent`.
