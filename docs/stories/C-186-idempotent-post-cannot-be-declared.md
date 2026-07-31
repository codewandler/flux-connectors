---
id: C-186
title: "An idempotent POST or PATCH cannot be declared, so two connectors ship a field that contradicts their own prose"
pillar: Spec
status: done
priority: 2
design: docs/designs/repeatable-writes.md
areas: [connector-spec, connector-flux]
note: "found twice in one wave: C-169's cache purge (POST, genuinely idempotent) and C-175's flag toggle (PATCH). check_write_metadata refuses `idempotent` on both methods, so each declares non_idempotent and documents the opposite in a comment. The declaration is what a host reads"
---

# An idempotent POST or PATCH cannot be declared, so two connectors ship a field that contradicts their own prose

## Goal

Let an operation declare the idempotency it actually has, so the field a host reads and the sentence a
human reads do not disagree.

## What was measured

`check_write_metadata` (`crates/connector-flux/src/lib.rs`) refuses `idempotency = "idempotent"` on
`POST` and on `PATCH`, by method, regardless of endpoint semantics. Two connectors in the same wave hit
it from different directions:

| operation | method | actually | declared | why |
|---|---|---|---|---|
| `cloudflare-cache-purge` ([C-169](C-169-provider-cloudflare.md)) | POST | idempotent — purging twice is purging once | `non_idempotent` | refused by method |
| `launchdarkly-flag-toggle` ([C-175](C-175-provider-launchdarkly.md)) | PATCH | idempotent — setting `on` to the same bit twice | `non_idempotent` | refused by method, citing RFC 9110 §9.2.2 |

Both implementors did the right thing: declared what the compiler allowed and documented the truth in a
comment. **But the comment is not what a host reads.** `ToolSpec` carries `idempotency`
(`flux-spec/src/lib.rs`), and a host deciding whether a retry is safe reads the field.

**The direction of the error is safe and that is exactly why it will not get fixed by accident.** An
under-claim makes a host more conservative about retries than it needs to be. Nothing breaks; a retry
that would have been fine is simply not attempted, forever, in silence.

## The decision this needs

Three options, and this wants a decision rather than a patch:

1. **Trust the author.** Drop the method-based refusal and let an operation declare `idempotent` on a
   POST. Cheapest, and it removes the guard that has probably prevented real mistakes — the RFC default
   is not idempotent for good reason, and most POSTs are not.
2. **Keep the refusal and add an explicit escape** — an author must say *why*, e.g.
   `idempotent_because = "purging an already-purged cache is a no-op"`, which the emitter records and a
   reviewer can read. The refusal still catches the careless case; the deliberate case becomes
   expressible and auditable.
3. **Do nothing and stop documenting the contradiction.** Accept that the field means "safe to retry
   *per the method*" rather than "idempotent", and rename it so it stops being a claim about the vendor.
   Also legitimate, and it would remove two misleading comments.

Option 2 is the likely answer, but the decision belongs in a story with a reason recorded.

## Acceptance

- [x] The decision is made and **recorded with its reasoning** in a design doc, not just in code.
      → [docs/designs/repeatable-writes.md](../designs/repeatable-writes.md). The answer is none of
      the three options as written: `conditional` — flux's own escape hatch — was always available
      and this repository had glossed it out of reach.
- [x] Whatever lands, `cloudflare-cache-purge` and `launchdarkly-flag-toggle` end up with a declared
      idempotency that does not contradict their own documentation — either the field changes or the
      documentation does.
      → the field changed, to `conditional`, with the condition stated. Two more instances were found
      and fixed (`miro-sticky-note-update`, and six pre-existing `conditional` operations that stated
      no condition at all).
- [x] **Failing-first test:** an operation declaring `idempotent` on POST is refused today; whatever
      replaces that behaviour is asserted.
      → `crates/connector-flux/tests/repeatability_condition.rs`.
- [x] If an escape hatch lands, a test asserts that the **careless** case is still refused. A guard that
      anyone can opt out of without saying why is not a guard.
      → `a_post_declaring_idempotent_is_still_refused_with_or_without_a_condition` (the refusal was
      restored to unconditional), `a_conditional_write_that_states_no_condition_is_refused`,
      `a_condition_that_says_nothing_does_not_unlock_the_claim`,
      `a_condition_of_pure_whitespace_does_not_clear_the_floor`, and
      `the_emitter_refuses_an_in_memory_ir` — which is the one that pins the emitter independently of
      the loader.
- [x] Every existing operation's emitted module is byte-identical unless it is one of the two named
      above, and those two are shown before and after.
      → three modules moved, not two; the six extra corrections moved none. Before and after in the
      Progress note.

## Notes

- Read the two connectors' header comments first — they are the case for the change, written by the
  people who hit it, and each cites the refusal at `path:line`.
- This is the *inverse* of the recurring class this repository keeps finding. C-152, C-151 and C-159
  were all cases where **the prose overclaimed what the code did**. Here the code under-claims what the
  vendor does and the prose is right. The lesson is the same either way: two statements of one fact
  drift, and only one of them is machine-checked.
- `PUT` and `DELETE` are presumably already allowed to be idempotent; confirm rather than assume, and
  say what the full method matrix is once you have read it.

## Progress

**Round 2 (rework). The first landing was wrong, and the way it was wrong is the finding.**

It added `idempotent_because` to let a `POST` declare `Idempotency::Idempotent`. Its own design doc
rejected the story's option 3 on the ground that *flux owns this vocabulary* — and then never asked
flux what the vocabulary means. `flux_spec::coherence` (pinned 1.3.0, linked by `connector-pack`)
declares **I3**: a consequence-bearing spec must not declare `Idempotent`, because that value licenses
flux's op cache to serve a stored result **instead of executing**, and it names `Conditional` as the
escape hatch for "safely repeatable".

Two measurements settled it:

1. **`conditional` on a `POST` always emitted, at the merge base, with nothing asked of it.** The
   story's premise — that a repeatable `POST` could not be declared — was false.
2. **This repository had narrowed the word.** `Idempotency::Conditional`'s doc said "idempotent only
   under a condition *the caller supplies* (e.g. an idempotency key)", and the refusal message
   repeated it. None of the three connectors has a caller-supplied key; their repeatability comes from
   what the endpoint does. So all three read `conditional` as unavailable and under-declared.

**The root cause was a gloss on a flux-owned value, not a missing feature.** So the rework is a
*tightening*, not a loosening:

- `idempotent` on `POST`/`PATCH` is refused **unconditionally** again — restored, not relaxed;
- a **mutating `conditional` must now state its condition** in `repeatable_because`, which is a rule
  that did not previously exist;
- the condition is refused where it means nothing, and is published to `web/public/catalog.json`.

Before and after, from `crates/catalog/ops/`:

| operation | before | after |
|---|---|---|
| `cloudflare-cache-purge` | `idempotency "non_idempotent"` | `idempotency "conditional"` |
| `launchdarkly-flag-toggle` | `idempotency "non_idempotent"` | `idempotency "conditional"` |
| `miro-sticky-note-update` | `idempotency "non_idempotent"` + a `description` explaining `check_write_metadata` | `idempotency "conditional"` + a description about sticky notes |

No other emitted module moved.

**Six more instances of the same defect, fixed at zero artifact cost.** `zendesk-ticket-update`,
`zendesk-ticket-comment-add`, `zendesk-ticket-tag-add`, `stripe-payment-intent-capture`,
`stripe-payment-intent-cancel` and `stripe-charge-refund-create` all declared `conditional` with the
condition in a TOML comment and in no field, artifact or manifest — three of them Stripe money
movements. Their comments moved into `repeatable_because`. Because that field reaches only
`catalog.json`, `build --provider zendesk` and `build --provider stripe` wrote **nothing**.

**flux conformance, measured over all 299 shipped operations** (`crates/connector-pack/tests/metadata_coherence.rs`):
I3 violations among `POST`/`PATCH` went 0 → 3 (first landing) → **0**. Two populations remain and are
filed rather than fixed: 192 *reads* trip I3 spuriously because every emitted operation declares
`effects ["network"]` with no `Effect::Read`; and **nine shipped `PUT`s claim `Idempotent`**, which
this repository permits under RFC 9110 §9.2.2 and flux refuses outright. That second one is a genuine
conflict — replaying a `PUT` is safe, *skipping* one is not — and is pinned by a two-way count so it
cannot grow unnoticed.

**Not a live hazard, and said plainly:** in flux 0.41 the only consumer of `Idempotent` is the op
cache, which also demands `Risk::Low` and read-only effects; all three operations fail those anyway.
This was honesty of declaration, not exploitability.

**Left open, deliberately.** `risk` has the identical method-shaped heuristic with no escape at all —
`notion-database-query` and `notion-search` are `POST` reads forced to `medium`, and C-110 measured
the whole-connector version for a GraphQL vendor. `risk` gates the *approval* path, so it needs its
own story. Also needs filing: emitting `Effect::Read` for non-mutating methods, the `PUT`/I3 conflict,
and `#[non_exhaustive]` on `Operation`.
