---
id: C-125
title: "Compose one input_schema per operation"
pillar: Codegen
status: ready
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

- [ ] `Operation::input_schema()` composes path, query, header and body parameters plus `body_schema`
      into one `object` schema with `properties` and `required`.
- [ ] **The merge rule `ir.rs:179` says is unstated is now stated and tested.** An operation with both
      `body_schema` and named body params either has a defined merge or is **refused at load** —
      decide which, record why in the design, and test it. Leaving two answers to one question is
      what this story exists to end.
- [ ] It is **derived, never authored**: no `input_schema` key in provider TOML, and one is a load
      error. Same rule as `Level` in
      [connector-configuration.md](../designs/connector-configuration.md).
- [ ] Parameter *wire* names are preserved. The composed schema keys by the caller-facing name the op
      declares, and the vendor's own spelling stays where it already lives — the split
      `crates/connector-flux/src/op.rs` already maintains for dotted names like `time.start`.
- [ ] `required` is exactly the operation's required parameters — not "everything", not "nothing".
- [ ] It reaches `catalog.json` under the every-key-always-present rule
      ([catalog-json.md](../designs/catalog-json.md)). Additive, so no `SCHEMA_VERSION` bump.
- [ ] **Failing-first test:** `every_operation_composes_an_input_schema_covering_its_parameters` —
      for all 97 shipped operations, assert every declared parameter appears in the composed schema
      and that required-ness matches. It must fail before composition exists.
- [ ] An operation with **no** parameters composes an empty object schema, not absence — "takes
      nothing" is a real answer, unlike "we don't know what it returns".
- [ ] The gate is green; the build stays a fixed point.

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
