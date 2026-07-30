---
id: C-128
title: "In and out shapes for events, channel bindings and graphs"
pillar: Codegen
status: ready
priority: 4
design: docs/designs/member-io-schemas.md
epic: member-io
areas: [connector-spec, codegen]
note: "the IR already carries the fields (inbound.rs, graph.rs) but nothing publishes them. A binding's 'out' is its reply operation's 'in' — the composition C-82 recorded, so reuse it rather than restating it"
---

# In and out shapes for events, channel bindings and graphs

## Goal

Publish what the non-operation members accept and produce, so a flow editor can type a wire into and
out of an event, a channel binding or a graph.

## Acceptance

- [ ] Each member kind publishes its in/out along the directions the design records — and the
      asymmetry is respected rather than forced into the operation's shape:

      | member | in | out |
      |---|---|---|
      | event | the vendor's inbound payload | — (an event returns nothing) |
      | channel binding | the inbound payload, **after** the payload map | the reply operation's input |
      | graph | its declared `inputs` ports | its declared `output` port |

- [ ] **A binding's "out" reuses its reply operation's composed input schema** from
      [C-125](C-125-composed-input-schema.md) rather than restating it. A binding is a *composition*
      of an event and a reply — restating the shape would create a second thing to keep in step.
- [ ] An event publishes **no** output. Modelling one would invite a consumer to wire a value out of
      something that produces none.
- [ ] The payload map's effect is visible: a consumer can tell the raw inbound payload from the mapped
      one, because those are the two different things a binding sits between.
- [ ] Absence is published as absence, per the epic's rule.
- [ ] **Failing-first test:** a binding whose reply operation's required input is not satisfiable from
      its declared payload map is **refused at load** — the "unbound required parameter" refusal C-82
      already promised, now checkable because both shapes are composed.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **Blocked on [C-83](C-83-channel-binding-codegen.md)**, which publishes events and bindings into the
  manifest and `catalog.json` at all. Do not start before it lands — there is no artifact to add a
  schema dimension to.
- Graph ports (`crates/connector-spec/src/graph.rs`) already carry an optional schema; events
  (`crates/connector-spec/src/inbound.rs`) do too. The fields exist and reach no artifact, which is
  the whole gap.
- Keep C-94's tripwire intact: no node or member field may hold a user-typed formula. Adding schemas
  must not become a back door for an expression.
- The graph half is only useful once graphs reach an artifact at all; if that has not happened, scope
  this story to events and bindings and say so rather than writing unreachable code.
