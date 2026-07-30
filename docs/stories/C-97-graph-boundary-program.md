---
id: C-97
title: The boundary program pattern — what an operator writes around a generated flow
pillar: Bridge
status: ready
priority: 5
design: docs/designs/flow-graph.md
epic: flow-graph
areas: [connector-cli, bridge]
note: "flux lifts only `op`; channel and trigger are Program members an operator writes. So a boundary node is DOCUMENTED, never emitted — the same split C-63 uses for poll"
---

# The boundary program pattern — what an operator writes around a generated flow

## Goal
Close the gap between a generated `op` and a running flow, without this repository emitting a Program
member it has no right to emit.

## Acceptance
- [ ] For each boundary node kind, generated documentation shows the exact program an operator writes:
      `trigger` → a `channel` matching the binding's transport plus `trigger on "<event>" run <graph>`;
      `schedule` → `channel schedule` with the declared cron; `endpoint` → the binding's channel.
- [ ] **Emitted nowhere.** A test asserts no `.flux` module contains `channel` or `trigger`, exactly
      as the member contract already requires for events and bindings.
- [ ] The documentation states what flux's schedule channel does **not** guarantee: it is best-effort,
      a restart drops ticks and replays none, so a `schedule` boundary must not be written as though a
      tick is certain. Same fact C-63 records for the poll transport.
- [ ] The graph's `inputs` are shown to line up with what the boundary delivers — a `trigger`
      boundary's op parameters come from the binding's payload map, so the two cannot silently drift.
- [ ] Records whether flux should eventually **load generated Program members**, and if so files it as
      a flux story per the C-16 / C-64 / C-84 handoff precedent. That is the decision that would let a
      graph ship as a complete app rather than an op plus instructions.

## Progress
- Not started. Depends on [C-95](C-95-graph-lowering.md).

## Notes
- This is the same strict split channel bindings hold: *"a binding declares; it never installs."*
- The honest framing for a user: a graph produces a callable operation, and two lines of program bind
  it to a trigger. If that second step proves to be the thing everyone gets wrong, that is the
  evidence for asking flux to load generated Program members.
