---
id: C-8
title: Emit a Flux op for a GET with path and query params
pillar: Codegen
status: ready
priority: 5
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
- [ ] An IR GET operation emits an `op` with typed params, a `description`/`risk`/`idempotency`/
      `effects`/`expose` metadata block, and a body that builds the URL and calls `http.request`.
- [ ] Emission goes through `flux_lang`'s AST and formatter — **not** string templates. A test
      asserts the emitter never produces output the formatter would reformat.
- [ ] Path parameters substitute into the URL; query parameters assemble into the request.
- [ ] Golden-file test pins the generated text for a fixture operation.
- [ ] IR types map to Flux types (`Number`, `String`, `Bool`, `Any`) with a documented fallback for
      shapes Flux cannot express.

## Progress
- (not started)

## Notes
- Composite op metadata keys are `description`, `risk`, `idempotency`, `effects`, `limits`, `expose`,
  `view` (`../flux/crates/flux-lang/docs/syntax.md:164`).
- `expose true` is what surfaces the op to the model as an LLM tool.
- Emit `risk`/`idempotency` honestly from the IR — a GET is `low`/`idempotent`, a POST is not.
