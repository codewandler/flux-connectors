---
id: C-413
title: "An operation can be callable without being an LLM tool"
pillar: Spec
status: done
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
- [x] `Operation` carries an exposure field. It **defaults to exposed**, so no shipped artifact moves —
      asserted by `cargo run -p connector-cli -- diff` reporting every artifact up to date after the
      change, with no regeneration.
      → `connector-spec/src/ir.rs:861` (`expose`), `ir.rs:1120,1133` (`exposed`/`is_exposed`), `src/graph.rs:462`;
      `diff` reports `557 artifacts up to date (53 providers checked)`, the same line as at the base.
- [x] `crates/connector-flux/src/op.rs:791` and `graph.rs:1182` read the field instead of the literal
      `true`. A failing-first test emits one unexposed operation and asserts the module declares
      `expose false`, and that it still parses and analyzes under C-11's gate.
      → `connector-flux/src/op.rs:795`, `src/graph.rs:1186`;
      `connector-flux/tests/exposure.rs::an_unexposed_operation_emits_a_module_declaring_expose_false`,
      whose `assert_gate` runs C-11's parse/canonical/load checks.
- [x] An unexposed operation is still **catalogued and callable**: it appears in the manifest's
      `operations` list, in `catalog.json`, and in the embedded catalogue, and `connector-pack` will
      still build a request for it. Only the `ToolSpec` projection is withheld.
      → `connector-cli/tests/exposure_artifacts.rs` (module, manifest, embedded catalogue, catalog.json);
      `connector-pack/tests/exposure.rs::an_unexposed_operation_composes_exactly_the_request_an_exposed_one_composes`
      and `::every_shipped_operation_resolves_whether_or_not_it_is_exposed`.
      **Two seams**: `pack` (model-facing, withholds) at `connector-pack/src/lib.rs:789`, and
      `resolve` (caller-facing, withholds nothing) at `lib.rs:842`, which
      `connectors-api/src/exec.rs:88` executes through.
- [x] The catalogue distinguishes the two states positively — a consumer can tell "not exposed" from
      "exposed", which is the distinction
      [C-235](C-235-the-catalogue-cannot-say-an-operation-is-public.md) needs and cannot express.
      → carried by the embedded Flux's `expose` line, which flux's formatter always writes and never
      elides, and read typed via `connector_pack::is_exposed` / `Rehearsal::is_exposed`. See Progress
      for why it is not a new column on `catalog::Operation`.
- [x] The provider TOML schema (`crates/connector-spec/schema/provider-toml.schema.json`) documents the
      key, and `tests/provider_schema.rs` holds it to what serde actually accepts.
      → schema `$defs.operation.properties.expose` and `$defs.graph.properties.expose`;
      `every_documented_object_lists_exactly_the_keys_the_loader_accepts` (which failed until both
      were added) plus `the_schema_publishes_the_exposure_default_the_loader_applies`.
- [x] The IR hash domain covers it, and `tests/ir_roundtrip.rs` proves an operation that does not use
      the field serializes exactly as it does today — every existing `ir_sha256` stays put.
      → `ir_roundtrip.rs::an_exposed_operation_serializes_exactly_as_it_did_before_the_field_existed`
      (byte equality against the pre-field encoding) and `an_unexposed_operation_reaches_the_hash_domain`.

## Progress
- **Landed.** `expose: bool` on both `Operation` and `Graph`, `#[serde(default = "exposed",
  skip_serializing_if = "is_exposed")]`. The two emitter literals now read the field. Gate green;
  `connector-cli -- diff` reports `557 artifacts up to date`, byte-identical to the base.
- **`Graph` got the field too.** The story names `graph.rs:1182` in its Acceptance, and that line
  lowers a *flow*, not an operation — there was nothing on `Graph` for it to read. Its `expose` is
  **authored, not derived**, unlike the `risk`/`idempotency` beside it: a curated flow over uncurated
  operations is the shape this story exists to allow, and deriving exposure from the called set would
  forbid exactly that.
- **The positive distinction is the Flux `expose` line, not a new catalogue column.** Adding a field
  to `catalog::Operation` or to `catalog.json`'s `OperationEntry` would have rewritten all 53
  generated catalogue modules and `catalog.json`, breaking "no shipped artifact moves" — the one
  requirement stated twice. The embedded Flux already states exposure positively on every operation
  (`flux_lang::format` writes `expose true`/`expose false` and elides neither), so the distinction is
  carried at zero artifact cost and read typed through `connector_pack::is_exposed`. If C-235 later
  wants a typed column, that is a whole-catalogue regeneration the coordinator runs, not this story.
- **`connector-pack` changed although `areas` does not list it.** Nothing else could withhold the
  tool: the pack registers native Rust `Tool`s and bypasses flux's own op machinery, so `expose` in
  the emitted Flux is inert until `install` consults it.
- **Corrected at review: filtering the registry alone made unexposed mean *uncallable*.** A
  `ToolRegistry` is both the advertisement surface (`specs`, which a host hands a model) and the
  resolution surface (`get`, which an execute route reads), so the first landing's filter withheld the
  call as a side effect of withholding the tool — and `connectors-api/src/exec.rs`, the only execute
  path in the workspace, resolved through exactly that registry. An unexposed operation was
  catalogued, documented, manifest-listed and unreachable, which is this story inverted and would have
  sunk the epic's premise (babelforce's ~388 callable-but-unexposed operations). The fix is a second
  seam: `connector_pack::resolve` projects and admits **one named operation regardless of exposure**,
  under the identical flux admission checks a packed tool passes, and `exec.rs` goes through it.
  `pack` stays model-facing. This also made `exec.rs` project one operation per request instead of
  every operation its provider ships.
- **Three tests re-cut that would have forbidden the feature.** `catalog/tests/embedded_operations.rs`
  asserted `meta.expose` of every embedded op; it now asserts the loaded flag **agrees with the
  emitted text**, which is a claim about renderings rather than about curation.
  `connector-pack/tests/projection.rs` asserted every operation resolves in the packed registry and
  that the counts match; both are now scoped to the exposed set. My own first-landing test
  "every shipped operation is exposed" had the same defect and was replaced by the two durable
  invariants: the registered set is exactly the exposed set, and **every** operation resolves.
- **Two golden error snapshots re-recorded** (`authored-input-schema`, `operation-auth-typo`): both
  embed the loader's accepted-key list, which now names `expose`. The new text is strictly better.
- **The `false` branch is tested where the type system allows.** `catalog::Operation` is
  `#[non_exhaustive]`, so no unexposed catalogue entry can be built outside the `catalog` crate and
  nothing shipped is unexposed yet. `spec::declares_exposure` — the exact predicate `pack` branches on
  — is unit-tested in both directions against a real rendering with one token changed, and a rendering
  that states no `expose` at all is pinned to **fail open** (`CompositeOpMeta::default()` is `true`),
  so the accidental failure mode is a tool staying visible rather than silently vanishing.
- Not done, and not this story's job: no shipped provider declares `expose = false` yet. Babelforce
  declaring the inverse is the payoff and belongs with the connector that needs it.

## Notes
- **This is a widening, not a loosening.** The default keeps today's behaviour; the new state is
  strictly more restrictive than what an author can express now.
- Naming matters more than usual: whatever the field is called becomes the word the explorer, the
  catalogue and a host's settings page all use. `expose` mirrors flux's own `op` metadata spelling and
  is the one the emitter already writes.
- Do **not** make this a curation mechanism in the loader — selection is C-411's job and stays opt-in.
  This field says what happens to an operation that was already selected.
- Independent of ingest: nothing here reads a spec. It can land before C-4.
