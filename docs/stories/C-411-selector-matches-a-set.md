---
id: C-411
title: "One selector selects many operations, so 397 do not cost 397 blocks"
pillar: Spec
status: in-progress
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
- [x] `[[patch.select]]` matches by `service`, `path_prefix` and `methods`, and every matched
      operation reaches the IR. A failing-first test selects `/api/v2/agents` + `GET` from a fixture
      and asserts the exact matched set.
      → `provider.rs` `OperationSelector` + `publish`;
      `tests/operation_selection.rs::a_selector_matches_by_service_path_prefix_and_method` (13 GETs,
      written out rather than counted). `path_prefix` matches on **whole segments** —
      `a_path_prefix_matches_on_segment_boundaries`.
- [x] **Selection stays opt-in.** No `hide` key, and an unmatched operation still does not reach the
      IR. A test asserts a spec-backed provider with no selector yields zero operations.
      → `there_is_no_hide_key`, `a_spec_backed_provider_with_no_selector_publishes_nothing`.
- [x] **A selector that matches nothing is a loud error**, for the same reason
      `OperationPatch::select` naming an absent operationId already is.
      → `a_selector_that_matches_nothing_is_refused`; the refusal reads back the statement itself.
- [x] A per-operation `[[patch.operations]]` block still works and **wins** over a selector that also
      matched it. Precedence is stated once, tested, and total.
      → stated once on `Patch`; **field by field** — a block's stated field wins, where the block is
      silent the selector's statement stands, where neither speaks each field's own rule decides or
      refuses. `a_per_operation_block_wins_over_a_selector`, `a_block_overrides_a_selectors_risk`.
- [x] Overlapping selectors are an error unless they agree — two statements silently fighting over one
      operation is how the merge order stops being total.
      → `Stated::absorb`/`agree`; silence is not disagreement, two *stated* values that differ are.
      `overlapping_selectors_that_agree_are_accepted`, `..._that_disagree_are_refused`.
- [x] Merge order stays fixed and byte-reproducible: spec → select → per-operation patch → validate,
      with a test asserting identical IR from identical inputs.
      → `identical_inputs_produce_identical_ir`. Published order: block-named operations in file
      order, then everything a selector matched in document order per `[[spec]]` entry — so a file
      with no selector publishes exactly what it published before selectors existed (557 artifacts
      unchanged).

## Progress
- Landed with C-412 and C-414 as one declaration; they all write
  `crates/connector-spec/src/provider.rs` and splitting them would have guaranteed conflicts.
- `expose` was added to the selector **and** to `OperationPatch`, which is not in this story. C-417
  needs it (388 unexposed operations is 388 lines declared per-operation) and it belongs to the same
  statement; the default stays exposed either way.
- **No operation on a path carrying an `internal` segment is ever selected.** Zero exist across the
  five documents today, so this is a guard against a future pull. A bulk statement excludes such a
  path silently; a `[[patch.operations]]` block naming one is refused, because that is an author
  asking for it. `an_internal_path_is_never_selected`.
- **Measured**: 392 operations selected from the five vendored documents in **175 declaration
  lines** (`crates/connector-spec/tests/fixtures/babelforce-canonical.toml`), of which 135 are the
  selection itself and 40 are the connector header C-410 already required. The 397 the scope
  constraint names is `392 + 5`: ingest skips five `multipart/form-data` uploads it cannot express,
  which is an IR gap rather than a selection gap and is asserted by path so it cannot drift.

## Notes
- Path prefix, not tag: `Manager` covers 309 of the manager document's 356 operations, while 47
  distinct three-segment path prefixes reproduce the SDK's 36 resource namespaces almost exactly.
- Sequenced after C-6 — the per-operation overlay is the thing a selector generalizes, so it must
  exist and be correct first.
