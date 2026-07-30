---
id: C-96
title: Map graph node ids to Flux AST paths, so a diagnostic lands on a canvas node
pillar: Bridge
status: ready
priority: 4
design: docs/designs/flow-graph.md
epic: flow-graph
areas: [connector-flux, connector-cli]
note: "flux ALREADY shipped this seam — D-139 added Diagnostic.node_path because a downstream graph canvas was parsing message text. Reuse it rather than reinventing"
---

# Map graph node ids to Flux AST paths, so a diagnostic lands on a canvas node

## Goal
When flux's analyzer rejects generated Flux, put the error on the node that caused it.

## Acceptance
- [ ] The lowering emits a `node-id → AST path` map alongside the module — `"notify"` →
      `"body[2].then[0]"`.
- [ ] The map is an artifact like any other: generated, committed, drift-checked.
- [ ] A test proves the map is total and correct — every graph node appears, and the path it names
      resolves to the statement that node produced.
- [ ] **Reuse flux's shipped seam.** `Diagnostic.node_path` (`crates/flux-lang/src/analyze.rs`) exists
      *because* a downstream graph canvas was parsing diagnostic message text — flux story D-139,
      status done, filed as *"ai-agent-platform's `NodeMap` keys analyzer findings back to
      graph-canvas nodes"*. Do not invent a second attribution mechanism.
- [ ] Round-trip: given a `node_path` from a diagnostic, the map yields the node id, and a test
      asserts both directions.

## Progress
- Not started. Depends on [C-95](C-95-graph-lowering.md), which produces the AST the paths index.

## Notes
- flux's own `NodeId` is a **positional index** into the body and is invalidated by any edit, which is
  why the path string is the stable locator and why this repository's node ids are author-owned.
- Worth checking with the ai-agent-platform canvas whether the map shape it wants matches this one
  before publishing it — the format is cheap to agree on now and expensive to change later.
