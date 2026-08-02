---
id: C-124
title: "Every member states what it receives and what it returns (epic)"
pillar: Codegen
status: ready
priority: 2
design: docs/designs/member-io-schemas.md
epic: member-io
areas: [connector-spec, codegen]
note: "EPIC — input composition and response-coverage floors landed; still open are truthful effective-output schemas and inbound member shapes. C-403 replaced the old flat string with a {status, headers, body} envelope, which is still not the vendor body schema"
---

# Every member states what it receives and what it returns (epic)

## Goal

Make every member with inputs publish, in JSON Schema, what it accepts and what it gives back — so a
UI can render a form, a flow editor can type a wire, and `ToolSpec` can be projected without
guesswork.

## Acceptance

- [x] Each operation publishes **one composed `input_schema`**, derived from its parameters and
      `body_schema` and never authored — [C-125](C-125-composed-input-schema.md).
- [x] `response_schema` coverage is **measured and non-decreasing**, with a ratcheted floor —
      [C-126](C-126-response-schema-coverage.md).
- [ ] The catalogue distinguishes **what the vendor sends** from **what a caller actually receives**,
      and never publishes the first as the second — [C-127](C-127-truthful-output-typing.md).
- [ ] Events, channel bindings and graphs publish their in/out shapes, with a binding's "out" reusing
      its reply operation's composed input rather than restating it —
      [C-128](C-128-inbound-io-schemas.md).
- [ ] **Absence is published as absence.** An operation with no declared response shape emits no
      schema — never `{}` or a permissive `{"type": "object"}`, which is indistinguishable from a real
      declaration and defeats the coverage measure.

## Children

- [C-125](C-125-composed-input-schema.md) — the composed `input_schema` (feeds C-114's `ToolSpec`)
- [C-126](C-126-response-schema-coverage.md) — raise and floor the response coverage
- [C-127](C-127-truthful-output-typing.md) — separate the vendor payload from the effective output
- [C-128](C-128-inbound-io-schemas.md) — events, channels and graphs (depends on C-83)

## Notes

**The historical measurement this epic was scoped from** (`web/public/catalog.json`, 97 operations): 92 carry
`parameters`, **16 carry `response_schema`**, 2 carry `body_schema`, and events/channels/graphs
appear zero times. The input side is largely present but uncomposed; the output side is largely
absent; the inbound side is unpublished.

**The trap, and the reason C-127 still exists.** `response_schema` describes the vendor's JSON body.
Since C-403 an emitted Flux op returns `http.request`'s `{status, headers, body}` envelope. Publishing
the body schema as the envelope's `output_schema` is still false: the correct path is `.body.data.id`,
not `.data.id`, and `body` may be a string for non-JSON content.

**Why now.** [C-114](C-114-tool-spec-projection.md) needs a composed input schema —
`ToolSpec.input_schema` is required, not optional — and the [Tool pack](../designs/connector-tool-pack.md)
now returns the same canonical record as the composite path. C-127 must describe that effective
envelope rather than assuming one surface parses into a different value.
