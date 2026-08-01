---
id: C-421
title: "No shipped provider can become spec-backed, because loading one is not spec-aware"
pillar: Spec
status: ready
priority: 1
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec, connector-cli]
note: "found by C-416 on 2026-08-01 and it blocks the epic outright — `provider::load` takes no spec cache, so a spec-backed provider loads as a ZERO-OPERATION SKELETON. 91 files call it, 86 of them tests. C-416 and C-417 are both stuck behind this"
---

# No shipped provider can become spec-backed, because loading one is not spec-aware

## Goal
Make loading a provider file mean the same thing everywhere, so converting a shipped provider to
`[spec]` does not silently turn it into a connector with no operations for most of the workspace.

## Acceptance
- [ ] **Plain `provider::load` on a spec-backed file no longer returns a skeleton.** It either
      resolves the spec cache or **refuses loudly** — a zero-operation connector that looks
      successfully loaded is exactly the "plausible but incorrect" outcome `AGENTS.md` refuses. Decide
      which, state the reasoning where a reader will find it, and make the failing-first test the one
      that proves the old behaviour was wrong.
- [ ] Every caller is accounted for. 91 files call `provider::load`; 86 are tests. Each either moves
      to the spec-aware entry point or keeps the pure one **deliberately**, and the split is
      explained once rather than per call site.
- [ ] `cargo test --workspace --no-fail-fast` is green with `providers/babelforce.toml` spec-backed.
      That is the whole point: C-416 measured **53 failures across 18 binaries in 4 crates** the
      moment one shipped provider converts.
- [ ] Tests that encode hand-authored babelforce shapes are rewritten against what the document
      actually declares, not deleted: `babelforce_ivr.rs::babelforce_nests_the_presence_label`,
      `::babelforce_sends_its_free_form_session_bodies`, and
      `connector-pack/tests/request.rs::a_free_form_body_travels_whole_in_either_spelling` (which only
      needs repointing at `babelforce-session-update`, still free-form).
- [ ] Any C-126 response-schema figure that moves is re-baselined **with the new number stated**, not
      silently relaxed — babelforce goes from 0/9 response schemas to 9/9, which is the floor rising.

## Progress
- 2026-08-01 — Filed from C-416's BLOCKED report. C-416's own branch (`impl/C-416`) is preserved and
  is the reproduction: it converts babelforce and shows exactly which binaries go red.

## Notes
- **This is the epic's critical path.** C-416 (reproduce the nine) and C-417 (full babelforce surface)
  are both blocked behind it, and C-420 multiplies it by every provider that converts.
- The design question the C-416 implementor surfaced, worth answering rather than routing around:
  `connector-spec` is deliberately pure (bytes → IR, no IO), and the spec cache is a **directory**.
  So either `load` grows a parameter carrying already-read documents — keeping purity — or the pure
  entry point stays and refuses a spec-backed file, with a second entry point for the full form. The
  first keeps one meaning for "load"; the second keeps one signature. Pick, and say why.
- Do **not** lift `validate_verify` to make the failures go away. 38 of the 53 trace to that one line,
  and the C-416 implementor tried it: `every_shipped_provider_loads` then correctly reports "declares
  no operations", which is the same defect one layer down.
