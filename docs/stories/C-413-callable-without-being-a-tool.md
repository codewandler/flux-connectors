---
id: C-413
title: "An operation can be callable without being an LLM tool"
pillar: Spec
status: ready
priority: 1
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec, connector-flux, connector-cli]
note: "`expose: true` is hard-coded at connector-flux/src/op.rs:791 and graph.rs:1182, so every emitted op is a tool. That is why babelforce ships 9 of 163 — and it is the one thing that makes 397 survivable"
---

# An operation can be callable without being an LLM tool

## Goal
Separate two claims the emitter currently fuses: that an operation **exists and can be called**, and
that it **reaches a model as a tool**. Without the split, a full-coverage connector is 397 tools.

## Acceptance
- [ ] `Operation` carries an exposure field. It **defaults to exposed**, so no shipped artifact moves —
      asserted by `cargo run -p connector-cli -- diff` reporting every artifact up to date after the
      change, with no regeneration.
- [ ] `crates/connector-flux/src/op.rs:791` and `graph.rs:1182` read the field instead of the literal
      `true`. A failing-first test emits one unexposed operation and asserts the module declares
      `expose false`, and that it still parses and analyzes under C-11's gate.
- [ ] An unexposed operation is still **catalogued and callable**: it appears in the manifest's
      `operations` list, in `catalog.json`, and in the embedded catalogue, and `connector-pack` will
      still build a request for it. Only the `ToolSpec` projection is withheld.
- [ ] The catalogue distinguishes the two states positively — a consumer can tell "not exposed" from
      "exposed", which is the distinction
      [C-235](C-235-the-catalogue-cannot-say-an-operation-is-public.md) needs and cannot express.
- [ ] The provider TOML schema (`crates/connector-spec/schema/provider-toml.schema.json`) documents the
      key, and `tests/provider_schema.rs` holds it to what serde actually accepts.
- [ ] The IR hash domain covers it, and `tests/ir_roundtrip.rs` proves an operation that does not use
      the field serializes exactly as it does today — every existing `ir_sha256` stays put.

## Progress
- (not started)

## Notes
- **This is a widening, not a loosening.** The default keeps today's behaviour; the new state is
  strictly more restrictive than what an author can express now.
- Naming matters more than usual: whatever the field is called becomes the word the explorer, the
  catalogue and a host's settings page all use. `expose` mirrors flux's own `op` metadata spelling and
  is the one the emitter already writes.
- Do **not** make this a curation mechanism in the loader — selection is C-411's job and stays opt-in.
  This field says what happens to an operation that was already selected.
- Independent of ingest: nothing here reads a spec. It can land before C-4.
