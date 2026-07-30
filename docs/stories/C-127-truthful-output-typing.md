---
id: C-127
title: "Separate what the vendor sends from what a caller receives"
pillar: Codegen
status: ready
priority: 3
design: docs/designs/member-io-schemas.md
epic: member-io
areas: [connector-spec, codegen, bridge]
note: "the trap this epic exists for — publishing response_schema as an output_schema means a consumer writing `.data.id` gets null on every call, with no error. http.request returns ONE FLAT STRING"
---

# Separate what the vendor sends from what a caller receives

## Goal

Make the catalogue tell the truth about output: the vendor's response body and the value a caller
actually receives are **different things**, and today they disagree for every single operation.

## Acceptance

- [ ] The catalogue publishes the two distinctly and never conflates them:
      - **`response_schema`** — what the vendor sends. Documentation. Honest today.
      - **the effective output** — what a caller receives. For the emitted `.flux` module this is a
        **string**, for every operation, without exception.
- [ ] The effective output is stated **per surface**, because it differs: the composite `.flux` path
      returns `http.request`'s flat string, while the [Tool pack](../designs/connector-tool-pack.md)
      can parse and return structured content. A single per-operation answer would be wrong for one
      of them.
- [ ] **Failing-first test:** `no_operation_publishes_the_vendor_body_as_its_flux_output` — assert
      that nothing in the generated artifacts presents `response_schema` as the value an emitted op
      returns. It must fail against an implementation that simply renames the field.
- [ ] The public site says this in prose where an operation's response is rendered, so a human reading
      the docs is not misled either.
- [ ] If the Tool pack lands a parsed output, the operation's `ToolSpec.output_schema` may carry
      `response_schema` — **but only for that surface**, and only where the pack genuinely parses.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **The concrete failure this prevents.** `http.request` returns `HTTP {status}\n{headers}\n{body}` as
  one flat string — the constraint `crates/connector-flux/src/op.rs` already records, and the reason
  error-envelope pointers live in prose there rather than in code. A consumer who reads a published
  output schema of `{"data": {"id": …}}` and writes `.data.id` against the emitted op gets `null` on
  every call, forever, and no error is raised. Silently broken flows are the worst outcome this
  repository can produce.
- This story is mostly **naming and honesty**, not machinery. Resist the urge to fix the underlying
  limitation here: making `http.request` return a record is a flux change, and if this analysis makes
  the case for it, file it on flux's board rather than working around it.
- Depends on nothing, but is most valuable **after** [C-126](C-126-response-schema-coverage.md) —
  the more response schemas exist, the more dangerous mislabelling them becomes.
- Coordinate with [C-115](C-115-request-delegation.md), which currently says to return the response
  as `ToolResult` content and leave shaping to a later story. This is that story.
