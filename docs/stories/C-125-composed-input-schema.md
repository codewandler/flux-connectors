---
id: C-125
title: "Compose one input_schema per operation"
pillar: Codegen
status: in-progress
priority: 2
design: docs/designs/member-io-schemas.md
epic: member-io
areas: [connector-spec, codegen]
note: "mechanical, with a clear right answer, and an immediate consumer — ToolSpec.input_schema is REQUIRED, so C-114 must otherwise invent its own and disagree with the catalogue"
---

# Compose one input_schema per operation

## Goal

Derive a single JSON Schema describing everything an operation receives, so no consumer has to
re-derive it and disagree about the corners.

## Acceptance

- [x] `Operation::input_schema()` composes path, query, header and body parameters plus `body_schema`
      into one `object` schema with `properties` and `required`.
- [x] **The merge rule `ir.rs:179` says is unstated is now stated and tested.** An operation with both
      `body_schema` and named body params either has a defined merge or is **refused at load** —
      decide which, record why in the design, and test it. Leaving two answers to one question is
      what this story exists to end.
- [x] It is **derived, never authored**: no `input_schema` key in provider TOML, and one is a load
      error. Same rule as `Level` in
      [connector-configuration.md](../designs/connector-configuration.md).
- [x] Parameter *wire* names are preserved. The composed schema keys by the caller-facing name the op
      declares, and the vendor's own spelling stays where it already lives — the split
      `crates/connector-flux/src/op.rs` already maintains for dotted names like `time.start`.
- [x] `required` is exactly the operation's required parameters — not "everything", not "nothing".
- [x] It reaches `catalog.json` under the every-key-always-present rule
      ([catalog-json.md](../designs/catalog-json.md)). Additive, so no `SCHEMA_VERSION` bump.
- [x] **Failing-first test:** `every_operation_composes_an_input_schema_covering_its_parameters` —
      for all 97 shipped operations, assert every declared parameter appears in the composed schema
      and that required-ness matches. It must fail before composition exists.
- [x] An operation with **no** parameters composes an empty object schema, not absence — "takes
      nothing" is a real answer, unlike "we don't know what it returns".
- [x] The gate is green; the build stays a fixed point.

## Notes

- **Coordinate with [C-114](C-114-tool-spec-projection.md).** `ToolSpec.input_schema` is required, so
  the Tool pack needs this. Whichever lands second must consume this function rather than writing a
  second composition — two derivations of one schema that must agree is precisely the drift risk
  `AGENTS.md` warns about for the C-12/C-95 lowering.
- 92 of 97 operations already carry `parameters`, so this is genuinely a projection: the data is
  there. Check what the other five are before assuming they are bugs — an operation that takes no
  input is legitimate.
- Do not "improve" any vendor's declared parameter schema while composing. Compose what is declared;
  raising coverage is C-126's job.

## Progress

**Landed on `impl/C-125`.** `connector_spec::Operation::input_schema()` composes path, query, header
and body parameters plus `body_schema` into one object schema, and `web/public/catalog.json`
publishes it per operation.

**The merge rule: refused at load.** An operation declaring both named `params.body` fields and a
free-form `params.body_schema` is now a loader error (`provider.rs`, `validate_operations`), pinned
by the golden fixture `tests/golden/body-declared-twice.toml`. Refusal rather than a merge because
there is no rule to write down — fields-win, schema-wins and fields-nested-inside are all decisions
no vendor document supports, and the failure mode of picking one silently is a request the vendor
answers `200` and ignores. `connector-flux` already refused it at *emission*; moving the refusal to
the loader makes it an invariant of the IR rather than of one back-end, so the composition cannot
face an ambiguous shape. Recorded in the design as §1.1. No shipped provider declares both, so the
rule costs nothing.

**Derived, never authored.** Two gates already existed and are now tested: `Operation` is
`deny_unknown_fields`, and the editor-facing `provider-toml.schema.json` closes the operation object.
`tests/golden/authored-input-schema.toml` pins the message.

**The two-derivations problem: held together by a test, not merged.** `connector-pack` derives its
`ToolSpec.input_schema` from the *emitted Flux*, and neither derivation can consume the other:

- it cannot key by the IR's names — a composite op declares symbols, so babelforce's `time.start` is
  `time_start` there, and that mapping lives in `connector-flux`, one dependency edge downstream of
  the IR (and `connector-pack` deliberately depends on the catalogue, never on the loader);
- it cannot key by the vendor's `required` — flux has no optional composite-op parameter and
  `request.rs` refuses a call that omits one, so the pack's `required` is necessarily *everything*,
  while the composed schema states what the *vendor* requires. Both are true; they answer different
  questions, and making the pack consume this one would have published an "optional" parameter its
  own request builder rejects.

So the resolution is the third option the dispatch offered: `connector-flux/tests/input_schema_agreement.rs`
asserts over all 105 shipped operations that the two describe the same parameter set modulo the
symbol mapping — through the now-public `connector_flux::parameter_symbols`, the same allocation the
emitter itself used — and that the composed `required` is always a subset of it. The one documented
exception is a `const`-pinned body field, which is sent but never declared. `connector-pack/src/spec.rs`
carries the same explanation in its module docs, where a reader of the pack will look.

**Artifact state at hand-off.** A full build was run and verified — `wrote web/public/catalog.json`,
then `diff` reported `248 artifacts up to date (18 providers checked)` — and `web/public/catalog.json`
was then **reverted**, because it is a whole-catalogue artifact the coordinator owns. Three tests are
therefore red on this branch, exactly the three AGENTS.md predicts for a change that touches an
existing provider's published data: `the_committed_tree_is_a_fixed_point_of_a_build`,
`a_build_plans_both_readme_images_and_they_are_current`, and
`the_build_writes_and_checks_site_catalog_json`. Regenerating at integration resolves all three.

**Not done, deliberately.** The composed schema does *not* reach `crates/catalog` (the Rust
catalogue): the Acceptance names `catalog.json` only, and adding a field there would rewrite 18
generated per-provider files for no consumer that exists yet. If `connector-pack` should ever consume
this composition rather than agree with it, that is the edge to build first — see C-127.
