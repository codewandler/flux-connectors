---
id: C-186
title: "An idempotent POST or PATCH cannot be declared, so two connectors ship a field that contradicts their own prose"
pillar: Spec
status: ready
priority: 2
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

- [ ] The decision is made and **recorded with its reasoning** in a design doc, not just in code.
- [ ] Whatever lands, `cloudflare-cache-purge` and `launchdarkly-flag-toggle` end up with a declared
      idempotency that does not contradict their own documentation — either the field changes or the
      documentation does.
- [ ] **Failing-first test:** an operation declaring `idempotent` on POST is refused today; whatever
      replaces that behaviour is asserted.
- [ ] If an escape hatch lands, a test asserts that the **careless** case is still refused. A guard that
      anyone can opt out of without saying why is not a guard.
- [ ] Every existing operation's emitted module is byte-identical unless it is one of the two named
      above, and those two are shown before and after.

## Notes

- Read the two connectors' header comments first — they are the case for the change, written by the
  people who hit it, and each cites the refusal at `path:line`.
- This is the *inverse* of the recurring class this repository keeps finding. C-152, C-151 and C-159
  were all cases where **the prose overclaimed what the code did**. Here the code under-claims what the
  vendor does and the prose is right. The lesson is the same either way: two statements of one fact
  drift, and only one of them is machine-checked.
- `PUT` and `DELETE` are presumably already allowed to be idempotent; confirm rather than assume, and
  say what the full method matrix is once you have read it.
