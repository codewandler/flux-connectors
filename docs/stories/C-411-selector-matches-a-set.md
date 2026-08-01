---
id: C-411
title: "One selector selects many operations, so 397 do not cost 397 blocks"
pillar: Spec
status: backlog
priority: 2
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec]
note: "`OperationPatch` selects exactly one operationId. Selection stays opt-in — this widens what one statement selects, it does not introduce `hide` and must not"
---

# One selector selects many operations, so 397 do not cost 397 blocks

## Goal
Let a provider select a set of operations in one declaration — by service, path prefix and method — so
a full-coverage connector is reviewable rather than 397 near-identical blocks.

## Acceptance
- [ ] `[[patch.select]]` matches by `service`, `path_prefix` and `methods`, and every matched
      operation reaches the IR. A failing-first test selects `/api/v2/agents` + `GET` from a fixture
      and asserts the exact matched set.
- [ ] **Selection stays opt-in.** No `hide` key, and an unmatched operation still does not reach the
      IR. A test asserts a spec-backed provider with no selector yields zero operations.
- [ ] **A selector that matches nothing is a loud error**, for the same reason
      `OperationPatch::select` naming an absent operationId already is.
- [ ] A per-operation `[[patch.operations]]` block still works and **wins** over a selector that also
      matched it. Precedence is stated once, tested, and total.
- [ ] Overlapping selectors are an error unless they agree — two statements silently fighting over one
      operation is how the merge order stops being total.
- [ ] Merge order stays fixed and byte-reproducible: spec → select → per-operation patch → validate,
      with a test asserting identical IR from identical inputs.

## Progress
- (not started)

## Notes
- Path prefix, not tag: `Manager` covers 309 of the manager document's 356 operations, while 47
  distinct three-segment path prefixes reproduce the SDK's 36 resource namespaces almost exactly.
- Sequenced after C-6 — the per-operation overlay is the thing a selector generalizes, so it must
  exist and be correct first.
