# Design: declaring a write that is safe to repeat

> Story: [C-186](../stories/C-186-idempotent-post-cannot-be-declared.md) ·
> Guard: `crates/connector-flux/src/op.rs::check_write_metadata` ·
> Field: `connector_spec::Operation::repeatable_because` ·
> Loader: `crates/connector-spec/src/provider.rs::validate_repeatability_condition` ·
> Artifact: `web/public/catalog.json` (`crates/connector-cli/src/site.rs`) ·
> Conformance: `crates/connector-pack/tests/metadata_coherence.rs`

## The defect, stated precisely

Before C-516, `check_write_metadata` refused `idempotency = "idempotent"` on `POST` and `PATCH` by
method. C-516 replaces that transport heuristic with authored vendor-state direction: every authored
write is refused `idempotent`, regardless of method, because Flux may skip consequence-bearing work.

Three shipped operations are nonetheless safe to repeat by their **vendors'** behaviour:

| operation | method | vendor behaviour | shipped as |
|---|---|---|---|
| `cloudflare-cache-purge` ([C-169](../stories/C-169-provider-cloudflare.md)) | `POST` | `purge_everything` names a target state, not a delta; a repeat empties an already-empty cache | `non_idempotent` |
| `launchdarkly-flag-toggle` ([C-175](../stories/C-175-provider-launchdarkly.md)) | `PATCH` | one JSON Patch `replace` onto one environment's `on` bit, an absolute value | `non_idempotent` |
| `miro-sticky-note-update` ([C-183](../stories/C-183-provider-miro.md)) | `PATCH` | the note's whole content is sent as an absolute value | `non_idempotent` |

Each implementor declared what the compiler accepted and wrote the truth in a comment. **The comment
is not what a host reads.** `idempotency` travels to flux's `ToolSpec`, and a host deciding whether a
retry is safe reads the field.

Two things kept this from being found by accident: the direction of the error is safe (an under-claim
only makes a host more conservative, silently, forever), and **the prose was right while the code was
wrong** — the inverse of C-151, C-152 and C-159, though the lesson is identical. Two statements of one
fact drift, and only one of them is machine-checked.

## The decision

The story offered three options and the answer is **none of them as written**. The story assumed the
only way to declare a repeatable `POST` was to relax the `idempotent` refusal. That assumption was
false, and checking it is what this design turned on.

### What the first landing got wrong

C-186 first landed as the story's option 2: keep the refusal, add `idempotent_because`, let a `POST`
declare `Idempotent` when it justified itself. That design rejected the story's option 3 on this
ground, which is correct and worth keeping:

> `idempotency` is not this repository's field: it is `flux_spec::Idempotency`, reaching flux's
> `ToolSpec`, and renaming a value away from its consumer's vocabulary would put a second meaning
> behind one name at the exact boundary where a host reads it.

Having established that **flux owns the vocabulary**, it then never asked flux what the vocabulary
means. `flux_spec::coherence` — pinned at 1.3.0, linked by `connector-pack` — declares **I3, the
repeatability floor**:

> A consequence-bearing spec must not declare `Idempotency::Idempotent`. … `Idempotent` is the claim
> "repeating this call is safe"; it is what **licenses the dispatcher's op cache to serve a stored
> result *instead of executing***. … For an op that mutates … the honest declaration is
> `NonIdempotent`, or `Conditional` when it is genuinely safe to repeat under stated conditions.
> **`Conditional` — not a loosened rule — is the escape hatch for "safely repeatable".**

That is decisive on its own terms. `Idempotent` is a stronger claim than "repeating is safe": it also
says *not running the call at all* is acceptable. A cache purge cannot make that claim — the cache
refills from origin between calls, so a stored result is stale by construction.

### Two facts that settled it

**1. `Conditional` was always permitted here, with nothing asked of it.** Measured:
a `POST` declaring `idempotency = "conditional"` and nothing else emitted cleanly at the merge base.
So the story's premise — that a repeatable `POST` could not be declared — was **false**. All three
connectors could have shipped the honest value from day one.

**2. They did not, because this repository had narrowed the word.** `connector_spec::Idempotency`'s
doc comment read *"idempotent only under a condition the caller supplies (e.g. an idempotency key)"*,
and the emitter's own refusal message repeated it: *"use `non_idempotent`, or `conditional` when the
caller supplies a key or stamp"*. None of the three has a key for a caller to supply — their
repeatability comes from what the *endpoint* does. So all three read `conditional` as unavailable and
reached past it to `non_idempotent`.

**The root cause was never a missing feature. It was a gloss on a flux-owned value, propagated into a
refusal message, that put three connectors outside a value always meant for them.**

### What landed instead

The mechanism — field, loader guard, emitter guard, artifact — is unchanged from the first landing.
What changed is which claim it licenses, and the direction of the change:

- **`idempotent` on an authored write is refused unconditionally.** No transport-shaped escape.
  The first landing weakened this; the rework restores it.
- **`conditional` on an authored write requires a stated condition.** This is a **tightening**
  of a rule that did not exist: the value was previously free. flux's wording is "safe to repeat under
  **stated** conditions", and nothing was making anyone state them.
- The three connectors declare `conditional` with their conditions stated.
- **Six pre-existing `conditional` operations gained the conditions they never had** — Zendesk's three
  safe-updates and Stripe's three money movements. Each already had the reasoning in a TOML comment;
  the change moves it into a field a host can read. This is the same defect the story was filed for,
  found in six more places, and it cost **zero** artifact churn (see below).

## The rules

| authored direction | `risk = "low"` | `idempotent` | `conditional` | `repeatable_because` |
|---|---|---|---|---|
| `read` | permitted | permitted | permitted | **refused** |
| `write` | refused | **refused** | permitted **with a stated condition** | required for `conditional` |

`repeatable_because` is refused three ways, each a distinct mistake, each with a golden-file snapshot
under `crates/connector-spec/tests/golden/`:

| written | refused because | fixture |
|---|---|---|
| on an authored read | nothing about a read repeats harmfully, so the field would spread as decoration until no reviewer read any of them | `repeatability-condition-on-a-get` |
| beside an `idempotency` that is not `conditional` | prose asserting what its own field denies — C-186's defect, backwards | `repeatability-condition-without-the-claim` |
| shorter than 24 characters after trimming | an escape hatch that costs nothing is a deleted guard wearing the guard's clothes | `repeatability-condition-says-nothing` |

and an authored `conditional` write **without** one is refused (`conditional-write-states-no-condition`).

**On the 24-character floor.** A floor on *effort*, not truth, calibrated rather than invented: it is
the length of `"purging twice is a no-op"`, the shortest honest reason anyone on this story wrote.
`MIN_REPEATABILITY_CONDITION` is the single definition — `tests/provider_schema.rs` reads *the
constant* to check the published JSON Schema's `minLength` rather than a second copy of the number,
because a hand-typed `24` in the schema would have re-enacted this story's own defect inside its fix.

The `trim` is load-bearing: without it, 24 spaces clear the floor and state nothing.

## It reaches an artifact

`web/public/catalog.json` carries the condition beside the claim it licenses:

```json
{
  "id": "cloudflare-cache-purge",
  "idempotency": "conditional",
  "repeatable_because": "`purge_everything` names a target state rather than applying a delta, …"
}
```

`null` for every operation that does not declare `conditional`, matching the document's existing rule
that every key is always present. `SCHEMA_VERSION` is not bumped — adding a field is additive for
every consumer that reads by name.

Three surfaces deliberately do **not** carry it. `connectors/*.flux`: `flux_lang`'s `CompositeOpMeta`
has a closed field set with no free-form slot, so it would need a flux-lang release.
`catalog::Operation`: a field there must be written into 299 struct literals across 53 generated
tables, and the condition is for a reviewer while the *value* already reaches `ToolSpec` through the
existing field. The manifest: it carries operation ids and no per-operation metadata, and `AGENTS.md`
refuses ad-hoc widening of it.

**This is also why correcting the six pre-existing operations was free.** `repeatable_because` reaches
only `catalog.json`, so `build --provider zendesk` and `build --provider stripe` wrote nothing —
verified, `10 artifacts up to date` and `11 artifacts up to date`. Six operations became honest at the
cost of zero artifact churn.

## Conformance to flux, measured

The original C-186 measurement exposed the conflict that C-516 resolves. Re-measured during C-516
across the current 829 operations:

| | before explicit direction | C-516 |
|---|---|---|
| authored directions | absent | **829 / 829** |
| direction/effect mismatches | not expressible | **0** |
| authored writes declaring `idempotent` | 9 | **0** |

The C-516 conformance proof is quantified over the whole catalogue. Every operation emits exactly one
direction effect beside `network`; every authored write is consequence-bearing and refuses
`idempotent`; the compatibility test executes Flux Flow's public canonical static gather admission.
HTTP method is deliberately absent from those predicates.

## Is any of this reachable today?

**Yes.** Flux Flow's static gather admission consumes effects, risk and idempotency. Connector
direction supplies only the authored read/write effect and staging hint; Flux's canonical predicate
continues to own the remaining admission criteria.

## Semver

`connector-spec` publishes as `codewandler-connector-spec`, live on crates.io since 2026-07-31.

- **A downstream provider-file author: additive with one narrow tightening.** The loader accepts a new
  optional key. The one file that loaded before and does not now is an authored `conditional` write with no
  stated condition — deliberate, and the whole point.
- **A downstream Rust consumer: breaking.** `Operation` is public and **not** `#[non_exhaustive]`, so
  adding a public field breaks struct-literal construction and exhaustive destructuring. Pre-1.0 that
  is the minor slot: **0.7.x → 0.8.0.**

[C-231](../stories/C-231-nothing-stops-a-secret-field-gaining-an-example.md) already forces 0.8.0, and
nothing here is worse than that covers. Nothing published is removed or renamed.
`MIN_REPEATABILITY_CONDITION` and `repeatable_because` are new names; the first landing's spellings
(`MIN_IDEMPOTENCY_JUSTIFICATION`, `idempotent_because`) never left this branch, so renaming them costs
a downstream author nothing.

Worth filing separately: **`Operation` should probably be `#[non_exhaustive]`**, which would make every
future field additive. It cannot be added quietly — `connector-flux`, `connector-cli` and this crate's
own tests all construct it literally and are all *outside* `connector-spec` for that attribute — so it
is its own change, cheapest in the same 0.8.0.

## What this does not fix

Risk follows authored direction rather than method. Some historically conservative operations such
as `notion-database-query` and `notion-search` remain authored writes pending individual semantic
review; their `POST` transport is neither evidence for nor against that declaration. Re-reviewing
those individual operations is separate connector curation, not part of this schema migration.
