---
id: C-8
title: Emit a Flux op for a GET with path and query params
pillar: Codegen
status: done
priority:
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-flux]
note: first end-to-end slice of codegen
---

# Emit a Flux op for a GET with path and query params

## Goal
Turn one IR operation into a formatted Flux `op` declaration by building real `flux_lang::ast` nodes,
establishing the emitter shape every later codegen story extends.

## Acceptance
- [x] An IR GET operation emits an `op` with typed params, a `description`/`risk`/`idempotency`/
      `effects`/`expose` metadata block, and a body that builds the URL and calls `http.request`.
- [x] Emission goes through `flux_lang`'s AST and formatter — **not** string templates. A test
      asserts the emitter never produces output the formatter would reformat.
- [x] Path parameters substitute into the URL; query parameters assemble into the request.
- [x] Golden-file test pins the generated text for a fixture operation.
- [x] IR types map to Flux types (`Number`, `String`, `Bool`, `Any`) with a documented fallback for
      shapes Flux cannot express.

## Progress
- **Landed** in `crates/connector-flux`: `emit_operation(&Connector, &Operation) -> Result<String>`
  lowers one IR operation into a `flux_lang::program::CompositeOpDecl` and hands it to
  `flux_lang::format::format_composite_op`. Three modules: `op` (the emitter and the emitted shape),
  `names` (wire-name ↔ symbol-name), `types` (JSON Schema → `TypeRef` and the `Any` fallback).
- Four golden files under `crates/connector-flux/tests/golden/`, drawn from
  [the operation inventory](../designs/provider-operation-inventory.md): no params, path + optional
  query, required + optional query, and dotted vendor names. Re-record with
  `UPDATE_GOLDEN=1 cargo test -p connector-flux`, then read the diff.
- The formatter fixed-point test is
  `emitted_text_is_a_fixed_point_of_the_flux_formatter`: the emitted text must parse with no errors
  *and* `flux_lang::format_cst::format_module` must return it unchanged.

### Three findings the next stories need

1. **A dotted op name cannot be *declared* in Flux.** `flux_lang`'s `decl_name` grammar admits only
   ASCII alphanumerics, `_` and `-`, and flux's own composite loader agrees
   (`../flux/crates/flux-flow/src/composites.rs:340`, *"is not filename-safe"*). Call sites accept
   dots; declarations do not. So `op zendesk.ticket.show` — the form in
   [connector-pipeline.md](../designs/connector-pipeline.md) and in
   [C-23](C-23-operation-naming-contract.md) — does not parse, in 0.37 or in flux's 0.38 tree. The
   emitter **refuses** such an id rather than rewriting it; **C-23 must pick the real form**, and the
   design's illustrative output needs correcting.
2. **flux has no optional composite-op parameter.** Every declared param is required at call time
   (`../flux/crates/flux-flow/src/registry.rs:183-184`: every param goes into `required_params`,
   `optional_params` stays empty). An IR parameter with `required: false` is therefore emitted as a
   declared param the caller may pass `null` for, with a `when` guard giving null the meaning "omit
   this filter". Truthiness-based, so a deliberate `0`/`false` also reads as absent.
3. **Nothing percent-encodes query values.** flux registers no URL-encoding op, so a value carrying
   a space, `&`, `#` or `=` corrupts the query string — which is exactly Zendesk's search syntax
   (inventory §3.3.5). Deliberately *not* half-fixed here; needs a flux-side op or a quirk story.

## Notes
- Composite op metadata keys are `description`, `risk`, `idempotency`, `effects`, `limits`, `expose`,
  `view` (`../flux/crates/flux-lang/docs/syntax.md:164`).
- `expose true` is what surfaces the op to the model as an LLM tool.
- Emit `risk`/`idempotency` honestly from the IR — a GET is `low`/`idempotent`, a POST is not.
- `flux_lang` does not re-export `flux_spec`'s `Risk`/`Idempotency`/`Effect`, and this crate must not
  take a direct `flux-spec` dependency to name them, so the metadata block reads their stable
  snake_case tags back through serde. `metadata_tags_are_the_ones_flux_lang_accepts` pins every tag
  the emitter can produce.
