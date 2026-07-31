---
id: C-186
title: "An idempotent POST or PATCH cannot be declared, so two connectors ship a field that contradicts their own prose"
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/idempotency-justification.md
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
      → [docs/designs/idempotency-justification.md](../designs/idempotency-justification.md),
      "The decision" — option 2, with option 1 and option 3 each rejected in writing.
- [x] Whatever lands, `cloudflare-cache-purge` and `launchdarkly-flag-toggle` end up with a declared
      idempotency that does not contradict their own documentation — either the field changes or the
      documentation does.
      → the field changed. `providers/cloudflare.toml`, `providers/launchdarkly.toml`; a third
      instance, `miro-sticky-note-update`, was found and fixed with them (see Progress).
- [x] **Failing-first test:** an operation declaring `idempotent` on POST is refused today; whatever
      replaces that behaviour is asserted.
      → `crates/connector-flux/tests/idempotency_justification.rs`.
- [x] If an escape hatch lands, a test asserts that the **careless** case is still refused. A guard that
      anyone can opt out of without saying why is not a guard.
      → `a_post_declaring_idempotent_without_a_reason_is_still_refused`,
      `a_patch_declaring_idempotent_without_a_reason_is_still_refused` and
      `a_reason_that_says_nothing_does_not_unlock_the_claim`; plus each of the three changed
      connectors re-asserts it on its own operation with the reason stripped.
- [x] Every existing operation's emitted module is byte-identical unless it is one of the two named
      above, and those two are shown before and after.
      → three operations moved, not two; every other emitted module is byte-identical
      (`diff --provider` reports no drift, and `git status` touched only those three providers'
      artifacts). Before and after are in the Progress note.

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

**Landed: option 2 — `idempotent_because`.** The method-based refusal is unchanged where an author
says nothing; stating a reason is what buys the claim, and the reason is published rather than left
in a comment. Reasoning, the rejected options, the full method matrix and the semver obligation are
in [docs/designs/idempotency-justification.md](../designs/idempotency-justification.md).

**The contradiction was found three times, not two.** The catalogue grew from 45 to 53 providers
after this story was filed, and `miro-sticky-note-update` (C-183) had hit the same wall. Its own
comment said *"the honest fix is C-186's escape hatch, not yet landed"*, so fixing it was this
story's instruction, left in place by its author. Worse, Miro had put the explanation in the
operation's `description` — the one string that reaches a **model** as its tool contract — so a fact
about this repository's compiler was shipping into `web/public/catalog.json` and
`crates/catalog/`, and became false the moment the rule changed. The description was rewritten to
describe Miro.

Before and after, from `crates/catalog/ops/`:

| operation | before | after |
|---|---|---|
| `cloudflare-cache-purge` | `idempotency "non_idempotent"` | `idempotency "idempotent"` |
| `launchdarkly-flag-toggle` | `idempotency "non_idempotent"` | `idempotency "idempotent"` |
| `miro-sticky-note-update` | `idempotency "non_idempotent"` + a `description` explaining `check_write_metadata` | `idempotency "idempotent"` + a description about sticky notes |

No other emitted module moved.

**`PUT`/`DELETE` confirmed rather than assumed** — they were already permitted to claim
`idempotent`, and the matrix in the design doc states all seven methods. Note that three shipped
deletes decline the claim anyway (`cloudflare-dns-record-delete`, `miro-sticky-note-delete`,
airtable's), because each vendor answers a repeat with `404` and documents no guarantee. That is a
connector declining to claim what it cannot back, and it is correct.

**Left open, deliberately.** `risk` carries the identical method-shaped heuristic with no escape at
all: `notion-database-query` and `notion-search` are `POST` **reads** forced to `medium`, and C-110
measured the whole-connector version for a GraphQL vendor where every operation is a `POST`. `risk`
gates flux's *approval* path rather than its retry path, so relaxing it is a safety change that
deserves its own story and its own evidence. It needs filing.
