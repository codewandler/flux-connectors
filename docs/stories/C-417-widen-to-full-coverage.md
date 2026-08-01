---
id: C-417
title: "Widen babelforce to manager-sdk's full coverage, and gate it"
pillar: Spec
status: in-progress
priority: 3
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [providers, connector-cli]
note: "397 operations against a catalogue that holds 299 across 53 providers today — so this more than doubles it, and every one of the four bulk declarations has to be carrying its weight for the file to stay reviewable"
---

# Widen babelforce to manager-sdk's full coverage, and gate it

## Goal
Take `providers/babelforce.toml` from nine operations to the 397 `manager-sdk/COVERAGE.md` calls
canonical, and put a check under it so coverage cannot regress unnoticed.

## Acceptance
- [x] **Scope is exactly what manager-sdk covers — owner-stated 2026-08-01: "only include things
      which are also included in manager-sdk itself".** That is `manager-sdk/COVERAGE.md`'s canonical
      **397**, which is the 398 operations the five documents declare minus
      `POST /api/v1/webhook/zendesk` (operationId `zendesk`) — a webhook *receiver* babelforce exposes
      for Zendesk to call, not a client-callable operation. Nothing beyond that set is added here.
- [x] **No `/api/internal` surface, ever.** Measured 2026-08-01: `internal` appears in **zero** paths
      across all five documents, so there is nothing to exclude today — which is exactly why this is a
      **guard** rather than a filter. A check refuses any selected operation whose path carries an
      `internal` segment, so a future spec pull cannot introduce one silently. Owner-stated, and the
      cost of the guard is one assertion.
- [x] All five services are selected and reach the IR; the emitted operation count matches the
      canonical scope, with every intentional exclusion named in the file beside its reason.
- [x] A coverage test in this repo mirrors `manager-sdk/COVERAGE.md`: it reads the vendored documents,
      counts operations, compares against what the connector emits, and **fails on a gap that is not
      explicitly allowed**. An allow-list entry requires a reason string.
- [x] The curated set stays exposed and the rest are callable-but-unexposed (C-413), so the tool
      catalogue does not grow by 388 entries. A test asserts the exposed count.
- [x] `providers/babelforce.toml` stays reviewable — if it has not stayed well under the ~6,000 lines
      hand-authoring would have cost, say so in Progress with the number, because that is the epic's
      thesis failing in the open rather than quietly.
- [ ] The explorer and `catalog.json` still render at this size. If they do not, file the finding
      against the C-99 explorer epic rather than trimming coverage to fit the UI.
- [x] 23 operations carry no `summary`/`description` in the documents. Each either gets one through
      the overlay or stays unexposed — a tool contract with no sentence in it does not ship.

## Progress

### The numbers, measured 2026-08-01

| | |
| --- | --- |
| Operations emitted | **392** |
| Operations exposed | **9** (the curated set, unmoved) |
| Canonical scope | **397** = 392 emitted + 5 multipart |
| Declared by the five documents | **398** = 397 + `POST /api/v1/webhook/zendesk` |
| `providers/babelforce.toml` | **710** lines, of which **247** are declarations |
| Artifacts | **949**, was 558 |
| `catalog.json` | **6.6 MB** |
| Undescribed operations | **23**, all unexposed |

**The epic's thesis, settled.** 247 declaration lines select 392 operations. One
`[[patch.operations]]` block per operation — the shape C-416 left behind — would have been north of
1,600 lines before a single real correction, and the story's own estimate for hand-authoring was
~6,000. Four declarations do the work: five `[[spec]]`, one `[patch.naming]` rule with nine pins,
thirteen `[[patch.select]]`, and ten `[[patch.operations]]` exceptions. The other 463 lines are
comments, which is the part of a connector definition that *should* grow with what it does.

### What was adopted rather than re-derived

`crates/connector-spec/tests/fixtures/babelforce-canonical.toml` (C-411) is the selection, and it
was taken essentially whole. **Three deliberate departures**, all in the same direction:

1. **The nine keep their shipped `risk` and `idempotency`.** The fixture stated only `expose = true`
   on each, so `updateAgentStatus`, `setCallSessionVariables` and `updateSessionVariables` would
   have taken the manager write selector's `high`/`non_idempotent` instead of the `medium`/
   `idempotent` they ship with. Those two fields reach a host's approval gate and its retry
   decision, so that would have been a silent behavioural change to three shipped tools dressed as
   a refactor. `babelforce_coverage.rs::the_nine_shipped_operations_keep_their_contract` now pins
   all five fields — id, method, path, risk, idempotency — so it cannot happen quietly again.
2. **The nine keep their `description`, `quirks.pagination` and `omit.query`.** The fixture was
   measuring line cost and dropped them; dropping `omit.query` would have handed a model a
   38-argument `babelforce-call-list` of which 18 arguments are duplicates of 18 others.
3. **`task-automation` and `task-schedule` state `api_version = "v3"`.** Both are `/api/v3`, and the
   connector-level `v2` is now a decision (it is the version the published address was minted
   under) rather than the observation it used to be.

### Two guards had to be corrected rather than satisfied

Both were sound while every provider was narrowly curated, and both produced a **false positive** at
392 operations. Neither was weakened: each was scoped to the namespace its claim is actually about.

- `service_units.rs` refused any planned path containing `-default.`, meaning the reserved service
  leaking into an installable unit's name. It matched `crates/catalog/ops/babelforce/babelforce-get-settings-for-audit-default.flux`,
  an operation named after the vendor's own `/api/v2/settings/audit/default`. Scoped to `connectors/`.
- `shipped_providers_build.rs::every_shipped_operation_reaches_its_module` refused a module
  containing `op <id>` for an id belonging to another service. `op babelforce-authorize-integration(`
  — a manager operation — prefix-matched the `auth` service's `babelforce-authorize`. Given the same
  two delimiters the positive half of that assertion already used.

### One real defect the widening surfaced

**24 operations arrive from the documents carrying a response schema that constrains nothing** —
almost all deletes answering `{"type": "object"}` with no properties, and `DELETE
/api/v2/outbound/lists/{id}/leads/{leadId}` answering a bare `{}`.

`response_schema_coverage.rs::no_operation_publishes_a_permissive_response_schema` has refused that
shape since C-126, on the grounds that a schema admitting every document tells a consumer no more
than absence while counting as a declaration. Everything it guarded was hand-written until this
story derived 392 schemas from five documents — so the gate met, for the first time, a placeholder
committed by *ingest* rather than by an author. Carrying it through would have laundered a vendor's
non-statement into this repository's coverage number.

Fixed at the source: `openapi::ingest` now drops a response schema that constrains nothing and
diagnoses the drop, and the predicate moved into the library as
`connector_spec::constrains_nothing` so the ingest refusal and the coverage gate are one rule rather
than two copies of it. **Zero shipped operations were affected when this landed** — the gate was
green, so no existing provider's output moves. It lowered measured coverage from 636/682 to 612/682,
which is the honest direction.

### Ratchets and hand-typed counts — measured, not moved

Coordinator-owned, so this story reports them and changes nothing:

| Constant | Recorded | Measured | Verified green at |
| --- | --- | --- | --- |
| `COVERED_FLOOR` | 277 | coverage is **612 of 682** (89%); babelforce 344/392 | **612** |
| `ABSENCE_CEILING` | 24 | absence is **70 of 682**; slack is 2, smallest provider is 2 | **71** (uniquely determined) |

Three tests are red on those two constants and nothing else:
`response_schema_coverage_does_not_fall_below_its_floor`, `the_recorded_floor_is_the_measured_figure`
and `a_connector_arriving_with_no_response_shapes_is_caught`. Both values were substituted, the
binary run green, and both reverted — so the pair above is verified rather than computed.

`AGENTS.md:116` states `558 artifacts up to date (53 providers checked)` as the gate's expected
line. It is now **949**.

### Open, and deliberately not closed here

- **`catalog.json` is 6.6 MB.** Every test that reads it passes and the build is a fixed point, but
  nothing in this repository renders the explorer in a browser, so "it still renders at this size"
  is **not** something this story verified. That acceptance box is left unticked rather than ticked
  on an untested claim — the finding belongs to the C-99 explorer epic, and coverage was not trimmed
  to fit a UI nobody measured.
- **The naming rule derives some poor ids.** `babelforce-list`, `babelforce-task`,
  `babelforce-tasks`, `babelforce-testing` and `babelforce-token` come from single-word
  `operationId`s in the task documents. All are legal, unique and unexposed, and pinning better
  spellings is a judgement per operation rather than a defect in the rule.
- **`connectors/babelforce.flux` and `.connector.toml` were deleted.** babelforce is no longer a
  default-only connector, so those two are orphans no service owns. Nothing in the build detects an
  orphaned artifact; that is worth a story.

## Notes
- Blocked behind C-416: do not widen until nine known-correct operations reproduce.
- Multipart is inexpressible (`BodyEncoding` is `Json | Form`), so five file-upload operations cannot
  be emitted at all. They are allow-list entries here and a named gap in C-418, not a silent absence.
- Expect this to surface real defects in the documents — 4 of the manager document's 356 operations
  publish no 2xx schema, and `task-schedule` publishes none at all. That is what the overlay is for.
