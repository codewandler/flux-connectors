---
id: C-6
title: Build the patch/overlay layer
pillar: Spec
status: backlog
priority:
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec]
note: "RE-CUT 2026-08-01 — C-4 could not land without applying the patch schema (an Operation needs id/risk/idempotency, which no spec carries), so four of the six bullets below arrived with it. What remains is the part that was always the bet: patching a REAL vendor document"
---

# Build the patch/overlay layer

## Goal
Finish the overlay where C-4 necessarily stopped, and answer the question the epic actually rests on:
can a patch set correct a real vendor document more cheaply than hand-writing the connector?

## Acceptance
- [x] **Selection is opt-in**: only operations a provider explicitly selects reach the IR. A spec
      with 400 endpoints must not yield 400 tools.
      → landed with C-4; enforced structurally at `crates/connector-spec/src/provider.rs:436`
      (an operation can only enter through `for patch in &loaded.patch.operations`), and proven by
      `everything_the_document_declares_stays_available_to_patch`.
- [x] A patch can rename an operation to a stable op id, override `risk`/`idempotency`/`description`,
      correct a parameter's type or requiredness — **except adding a parameter the spec omitted**,
      which is still absent: `correct()` refuses an unmatched name. That half stays open below.
- [x] Quirks attach per operation: pagination style, rate limit, error envelope. → landed with C-4.
- [x] Merge order is fixed and total (**spec → patch → validate**), with a determinism test.
      → landed with C-4.
- [x] A patch that targets an operation absent from the spec is a loud error, not a silent no-op.
      → landed with C-4.
- [ ] **A patch can add a parameter the vendor document omits.** Real specs are incomplete; this is
      the one correction the schema promises and the code does not yet perform.
- [ ] **Zendesk's real spec plus a patch set reproduces the operation set of the zendesk plugin.**
      This is the story's whole remaining point. Only a 4-operation excerpt is committed today, so
      nothing has yet measured patch ergonomics against a full vendor document.
- [ ] **The cost is reported as a number, not an impression.** Lines of patch TOML per operation
      against the hand-authored equivalent, written into Progress — because "patching is harder than
      hand-writing" is a claim this story exists to settle, and it cannot be settled by feel.

## Progress
- 2026-08-01 — **Re-cut after C-4's review.** C-4 had to apply `select`/`rename`/`risk`/`idempotency`
  to satisfy its own acceptance at all, and applied `description`/`auth`/`quirks`/`params` rather
  than parse declared fields and ignore them. An independent reviewer confirmed nothing here is
  foreclosed: adding an omitted parameter is genuinely absent, and Zendesk parity is untouched.

## Notes
- **The bet did not go away, it moved.** What made this "the highest-risk story" was never the merge
  mechanics — it was whether a bad vendor document can be patched more cheaply than it can be
  hand-written. That question is now concentrated in the last three bullets, and babelforce's 398
  operations (C-417) is where it gets answered at scale.
- Op ids are a **public contract** (users and models call them). They must be stable across
  regeneration and must not derive from volatile spec fields like `operationId` without a pinned
  override.
