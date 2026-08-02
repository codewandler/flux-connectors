---
id: C-127
title: "Separate what the vendor sends from what a caller receives"
pillar: Codegen
status: ready
priority: 3
design: docs/designs/member-io-schemas.md
epic: member-io
areas: [connector-spec, codegen, bridge]
note: "C-403 changed the effective value to {status, headers, body}; the story remains because response_schema describes body, not that envelope, and publishing it as output_schema still gives callers the wrong path"
---

# Separate what the vendor sends from what a caller receives

## Goal

Make the catalogue tell the truth about output: the vendor's response body and the value a caller
actually receives are **different things**. Since C-403 the latter is a record rather than a flat
string, but it is still an envelope around the former.

## Acceptance

- [ ] The catalogue publishes the two distinctly and never conflates them:
      - **`response_schema`** — what the vendor sends. Documentation. Honest today.
      - **the effective output** — the `{status, headers, body}` record a caller receives, with the
        vendor schema applying below `body` only when that body is JSON.
- [ ] The effective output is stated **per surface**, even where the surfaces currently agree:
      both the composite `.flux` path and the [Tool pack](../designs/connector-tool-pack.md) return
      flux-web's canonical record unchanged. A future divergence must be explicit rather than folded
      into one per-operation answer.
- [ ] **Failing-first test:** `no_operation_publishes_the_vendor_body_as_its_flux_output` — assert
      that nothing in the generated artifacts presents `response_schema` as the value an emitted op
      returns. It must fail against an implementation that simply renames the field.
- [ ] The public site says this in prose where an operation's response is rendered, so a human reading
      the docs is not misled either.
- [ ] `ToolSpec.output_schema`, if projected, describes the effective record and nests the vendor's
      `response_schema` under `body`; it never renames the body schema into the whole result.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **The concrete failure this prevents.** A consumer who reads the body schema
  `{"data": {"id": …}}` as the operation output and writes `.data.id` gets `null`; the effective
  path is `.body.data.id`. Non-JSON content makes the distinction sharper because `body` remains a
  string. Silently broken flows are the worst outcome this repository can produce.
- C-403 supplied the record this story used to ask flux for. The remaining work is naming, schema
  composition and documentation in this repository, not response machinery.
- Depends on nothing, but is most valuable **after** [C-126](C-126-response-schema-coverage.md) —
  the more response schemas exist, the more dangerous mislabelling them becomes.
- Coordinate with [C-115](C-115-request-delegation.md), which currently says to return the response
  as `ToolResult` content and leave shaping to a later story. This is that story.
