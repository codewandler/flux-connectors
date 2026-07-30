---
id: C-39
title: Emit synthetic describe and schema operations
pillar: Codegen
status: ready
priority: 7
design: docs/designs/connector-bundle.md
epic: connector-bundle
areas: [connector-flux]
note: metadata reachable from inside a flux session, via the mechanism that already exists
---

# Emit synthetic describe and schema operations

## Goal
Make a connector self-describing from inside a flux session: two pure operations that return the
connector's metadata and its schemas, without adding any new flux concept.

## Acceptance
- [ ] `<provider>-describe` returns vendor, addresses, the credentials required, the host allowlist
      and the operation list. Small enough to call freely.
- [ ] `<provider>-schema` returns full **input** JSON Schema per operation, and whatever **output**
      schema exists.
- [ ] Both are **pure**: `effects []`, no `http.request`, no IO. The body is a literal record.
- [ ] Both are `expose false` — registered and callable, but out of the model's tool catalog, so they
      cost no context until something asks.
- [ ] Both pass the same parse-and-analyze gate and formatter fixed-point test as every other op.
- [ ] Generated deterministically; a rebuild from unchanged inputs is byte-identical.

## Progress
- (not started)

## Notes
- **Why a synthetic op rather than a metadata declaration.** flux modules *can* carry structured data
  — `datasource`/`channel` decls collect unknown keys into a nested `settings` record
  (`../flux/crates/flux-lang/docs/syntax.md:130-150`). But that borrows a declaration kind that means
  something else to flux, and `DynamicComposites::load` only lifts `op` declarations anyway. A pure
  op rides the mechanism that already works and is introspectable through the same interface as
  everything else.
- **Two ops, not one**, because they are consulted at different times and differ enormously in size.
- **Output schemas are the weak link.** `Operation::response_schema` exists but nothing populates it
  richly (flagged by C-9 and C-17), so `schema` returns complete inputs and mostly-empty outputs.
  Say so in the op's description rather than implying fidelity that is not there.
- **Measure `schema` before shipping it for a large provider.** Full JSON Schema for 25 operations as
  a literal record is already big; a 163-operation provider may need per-group rather than
  per-provider.
- A synthetic op is still an op: it occupies a name, and obeys C-23's naming rules and C-37's
  addressing like any other.
