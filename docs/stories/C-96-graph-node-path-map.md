---
id: C-96
title: Map graph node ids to Flux AST paths, so a diagnostic lands on a canvas node
pillar: Bridge
status: in-progress
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
- [x] The lowering emits a `node-id → AST path` map alongside the module — `"notify"` →
      `"body[2].then[0]"`.
- [ ] The map is an artifact like any other: generated, committed, drift-checked.
- [x] A test proves the map is total and correct — every graph node appears, and the path it names
      resolves to the statement that node produced.
- [x] **Reuse flux's shipped seam.** `Diagnostic.node_path` (`crates/flux-lang/src/analyze.rs`) exists
      *because* a downstream graph canvas was parsing diagnostic message text — flux story D-139,
      status done, filed as *"ai-agent-platform's `NodeMap` keys analyzer findings back to
      graph-canvas nodes"*. Do not invent a second attribution mechanism.
- [x] Round-trip: given a `node_path` from a diagnostic, the map yields the node id, and a test
      asserts both directions.

## Progress
- `connector_flux::emit_graph_with_paths` returns `(String, NodePaths)` — the module and the map
  beside it. `emit_graph` is now a thin wrapper, so the 20 existing call sites are untouched.
- The paths are **recorded as the statements are pushed**, in the emitting walk itself, rather than
  counted afterwards. The case that makes this non-optional is a gate comparing against a literal:
  the literal is bound as a statement of its own *ahead* of the `when`, so every path in that block
  shifts by one (`a_gates_bound_literal_shifts_the_paths_after_it`).
- **Totality has one documented hole, and it is the honest one.** A boundary node — `trigger`,
  `schedule`, `endpoint` — becomes a *parameter* of the emitted op and reaches no statement. flux
  renders no path for a parameter, so the map has no entry for one; spelling a `params[0]` would be
  inventing the second attribution mechanism this story exists to avoid. The test asserts every
  absence is a boundary and that nothing else is missing.
- `NodePaths::node_at` answers the reverse direction, matching on **whole segments** and taking the
  innermost statement, because flux descends *into* a statement: `body[1].body[0]` is the bind and
  `body[1].body[0].value` is the call it binds, and both belong to the same node. The round-trip
  test drives flux's own `analyze_flow` over the emitted op and keys real `Diagnostic.node_path`
  values back.
- **The artifact half is not done, and cannot be here.** Nothing in `providers/` declares
  `[[graphs]]`, and `connector-cli` never calls `emit_graph` at all — so `build` writes no graph
  module for a map to sit beside, and there is no repository artifact to commit or drift-check.
  What landed instead: `NodePaths` serializes (`to_json`, `Serialize`/`Deserialize`) and the two
  fixture graphs' maps are committed as goldens (`tests/golden/graph-*.paths.json`), so the shape is
  pinned and drifts loudly. Writing `connectors/<provider>-<graph>.paths.json` belongs with whichever
  story first makes `build` emit a graph module — see [C-97](C-97-graph-boundary-program.md) and the
  `[[graphs]]` row of AGENTS.md's "Intentional gaps" ("no consumer *and* no producer").

## Notes
- flux's own `NodeId` is a **positional index** into the body and is invalidated by any edit, which is
  why the path string is the stable locator and why this repository's node ids are author-owned.
- Worth checking with the ai-agent-platform canvas whether the map shape it wants matches this one
  before publishing it — the format is cheap to agree on now and expensive to change later.
