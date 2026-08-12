---
id: C-552
title: "The document carries the caller's contract, so the emitter can retire"
pillar: Build
status: ready
priority: 2
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-cli, connector-pack, catalog]
note: "C-538's measured residue: the ToolSpec projection still parses emitted Flux because the document lacks the caller-facing symbol, the error-envelope-extended description, and the contract input_schema. Until the document carries them, C-540 cannot delete the emitter — the differential gate compares against the emitted declaration, and deleting the emitter deletes the thing holding the reproduction honest"
---

# The document carries the caller's contract, so the emitter can retire

## Goal

Close the gap C-538 measured and recorded: three things a model or caller receives are still
recovered by parsing emitted Flux, because the canonical document does not carry them —

1. **the caller-facing symbol** — the document publishes a parameter's IR `name` (`time.start`,
   `$top`) and `wire` name, but the pack's contract advertises the Flux symbol (`time_start`,
   `_top`, `response_2`); `connector-resolve` reproduces the allocator, held honest only by the
   differential gate's comparison against the emitted declaration;
2. **the error-envelope-extended description** — the ToolSpec text appends the error-envelope
   paragraph the document's one-line summary lacks (measured on `airtable-record-get`);
3. **the contract `input_schema`** — `flux_lang::OpSpec::lower` over Flux types, where the
   document carries the vendor's JSON Schema (`int64 integer` vs `number`).

Widen the document (additive schema evolution per the C-537 forward-compat contract) so the
projection reads the artifact, extend the differential gate to cover the ToolSpec, and thereby
remove the last `parse_str` from `connector-pack` — the precondition C-540's deletion actually
waits on.

## Acceptance

- [ ] The canonical document carries the caller-facing symbol per parameter, the projection
      description, and the contract `input_schema` — additive fields, minor schema bump, every
      committed document regenerated (a whole-catalogue change: coordinator regenerates at
      integration).
- [ ] The differential gate extends to the `ToolSpec`: document-derived projection byte-identical
      to the Flux-derived one for every operation — description, `input_schema`, exposure — with a
      seeded divergence proving it can fail.
- [ ] `spec.rs`'s `parse_str` becomes unreachable from `connector-pack` entirely; the
      symbol-allocator reproduction in `connector-resolve` either retires (the document states the
      symbol) or is demoted to a validated read.
- [ ] C-538's ADJACENT 2 trap is closed or proven unreachable: a `const`-pinned body field whose
      name normalizes onto a later parameter's symbol must not shift names between the emitted
      declaration and the document (the emitter allocates for every body param; the document
      omits const-pinned ones).
- [ ] The `format = "origin"` blind spot is closed (C-538's review, open question 3): the deleted
      `Operation::project` override was never carried by either side of the differential gate, so
      a future provider declaring `format = "origin"` for a variable inside a larger authority
      (`https://{v}.x/`) would silently drop Origin→Host with nothing red. A loader or gate
      assertion requires every `format = "origin"` field's bound variable to lower to `["origin"]`
      in the document.
- [ ] The redaction row's nature is stated where the gate documents itself (C-538's review, open
      question 2): it is a restatement — plan and expectation share `placed_form` — not a
      two-derivation differential; deliberate per the design, but the gate's table must not read
      as if it were compared.
- [ ] `docs/designs/catalog-artifact.md` §3 records the closure, and C-540's story gains the
      explicit dependency note.

## Progress

- 2026-08-12: Filed at C-538's integration from its DEVIATION 2 and ADJACENT 1–3 findings.

## Notes

- Write set: `crates/connector-cli/src/document.rs` (+ schema), `crates/connector-resolve`,
  `crates/connector-pack`, every `catalog/*.catalog.json` (coordinator-regenerated), the design
  doc. Collides with C-548 (connector-pack comments) and any catalog-artifact story; do not share
  a wave with either.
- The description decision has a design edge: carrying the extended text in the document means
  the document states host-envelope behaviour. Decide deliberately whether that text belongs to
  the artifact or to the projection layer, and record why in the design doc.
