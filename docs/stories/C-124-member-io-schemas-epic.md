---
id: C-124
title: "Every member states what it receives and what it returns (epic)"
pillar: Codegen
status: ready
priority: 2
design: docs/designs/member-io-schemas.md
epic: member-io
areas: [connector-spec, codegen]
note: "EPIC — measured, not assumed: 92/97 operations carry parameters but only 16/97 carry a response_schema, and events/channels/graphs reach no artifact at all. The trap: response_schema is the VENDOR's body, while an emitted op returns one flat string"
---

# Every member states what it receives and what it returns (epic)

## Goal

Make every member with inputs publish, in JSON Schema, what it accepts and what it gives back — so a
UI can render a form, a flow editor can type a wire, and `ToolSpec` can be projected without
guesswork.

## Acceptance

- [ ] Each operation publishes **one composed `input_schema`**, derived from its parameters and
      `body_schema` and never authored — [C-125](C-125-composed-input-schema.md).
- [ ] `response_schema` coverage is **measured and non-decreasing**, with the current 16% recorded as
      the floor — [C-126](C-126-response-schema-coverage.md).
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

**The measurement this epic is scoped from** (`web/public/catalog.json`, 97 operations): 92 carry
`parameters`, **16 carry `response_schema`**, 2 carry `body_schema`, and events/channels/graphs
appear zero times. The input side is largely present but uncomposed; the output side is largely
absent; the inbound side is unpublished.

**The trap, and the reason C-127 exists.** `response_schema` describes the vendor's JSON body. An
emitted flux op returns `http.request`'s flat string — `HTTP {status}\n{headers}\n{body}`. Publishing
the former as an `output_schema` means a consumer writing `.data.id` gets `null` on every call, with
no error. That is a documentation bug that produces silently broken flows, and it is the single
thing this epic most needs to get right.

**Why now.** [C-114](C-114-tool-spec-projection.md) needs a composed input schema —
`ToolSpec.input_schema` is required, not optional — and the [Tool pack](../designs/connector-tool-pack.md)
is what finally makes a *true* `output_schema` possible, since a Tool can parse the body and return
structured content while the composite path cannot.
