---
id: C-426
title: "`multipart/form-data` is inexpressible, and it is the last five operations between babelforce and full parity"
pillar: Spec
status: ready
priority: 1
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec, connector-flux]
note: "measured by the C-411 selector 2026-08-01 — the canonical selection reaches 392 of 397, and the missing five are ALL multipart uploads ingest skips. `BodyEncoding` is `Json | Form` and has no third value. This is now the only thing standing between babelforce and manager-sdk parity"
---

# `multipart/form-data` is inexpressible, and it is the last five operations between babelforce and full parity

## Goal
Give `BodyEncoding` a `multipart` variant so a file-upload operation can be described and emitted,
closing the last gap between the babelforce connector and manager-sdk's canonical 397.

## Acceptance
- [ ] `BodyEncoding` gains a multipart variant, and `crates/connector-spec/src/openapi.rs` stops
      skipping an operation whose request body is `multipart/form-data`.
- [ ] The emitter produces something a caller can actually use, or the story stops and says why.
      **This is the real risk and it must be faced, not routed around**: flux's `http.request` takes a
      body, and if it cannot express a multipart part with a filename and a content type then the IR
      can describe the operation and the module still cannot perform it. Establish that first, against
      the flux version this repo pins, and report the finding before writing an emitter.
- [ ] The five babelforce operations are the acceptance set, named because ingest already names them:
      `POST /api/v2/agents/provision`, `POST /api/v2/agents/provision/validate`,
      `POST /api/v2/outbound/lists/{id}/leads/upload`, `POST /api/v2/phonebook/bulk`,
      `POST /api/v2/prompts`.
- [ ] **The accounting test flips rather than being edited.**
      `crates/connector-spec/tests/operation_selection.rs` asserts `392 + 5 = 397` and pins the five by
      path. When this lands, that test should go red *because the five moved*, and the fix is to move
      them from the skipped list to the selected count — not to relax the assertion.
- [ ] A nested body under multipart is refused rather than guessed, the same way `BodyEncoding::Form`
      already refuses one: form has no agreed nesting convention and neither does this.

## Progress
- (not started)

## Notes
- **This is the goal's last blocker.** The owner's standing instruction is the full manager-sdk
  surface; C-417 can deliver 392 of 397 today with these five allow-listed, and only this story closes
  the remainder.
- The gap has been known since the epic was planned — `docs/designs/spec-front-end.md` names multipart
  first under "What retiring manager-sdk actually requires" — but it was 5 of 398 in a design document
  and is now the whole distance between a connector and its parity claim.
- Sequenced **after** [C-417](C-417-widen-to-full-coverage.md), which lands the 392 and the allow-list.
  Landing this first would mean regenerating babelforce twice.
- If the answer turns out to be "flux cannot carry a multipart body", that is a legitimate outcome:
  record it, keep the five allow-listed with the reason, and file the upstream story. A connector that
  says honestly what it cannot do beats one that emits a module that fails at runtime.
