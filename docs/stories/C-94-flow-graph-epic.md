---
id: C-94
title: "The flow graph — connector members composed into one Flux op (epic)"
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/flow-graph.md
epic: flow-graph
areas: [connector-spec, connector-flux, bridge]
note: "EPIC — four waves built the vocabulary (Operation=call node, EventDecl=source, oip=node id, wire paths=edges); this is the graph. NOT a second language: every past rejection was an EXPRESSION language, every acceptance was declarative structure. IR landed"
---

# The flow graph — connector members composed into one Flux op (epic)

## Goal
Let a connector compose its own members into a flow — operation, gate, operation — and ship the schema
a flow editor renders, without inventing a language in front of Flux.

## Acceptance
- [x] **The principle-2 question is answered with evidence, not assertion.** Every past rejection was
      an expression language; every acceptance was declarative structure. No node carries a formula,
      and `NodeKind::free_text` is the exhaustive tripwire that keeps it true.
- [x] The IR: author-stable node ids, typed ports, edges, regions, and a closed `Condition`.
- [x] **Structural rules Flux's own semantics dictate** — no cycles, no edge leaving a region except
      through a declared port, and a `Gate` exporting nothing because `when` has no else here.
- [x] Every node reference resolves to a member of the graph's own service; graphs join the shared
      member namespace; a graph is in the hash domain.
- [x] Boundary nodes (`trigger`, `schedule`, `endpoint`) take no inputs and sit in no region.
- [ ] Lowering to a composite `op` — [C-95](C-95-graph-lowering.md).
- [ ] The node-id ↔ AST-path map — [C-96](C-96-graph-node-path-map.md).
- [ ] The operator program pattern — [C-97](C-97-graph-boundary-program.md).
- [ ] The richer node kinds flux-lang already has — [C-98](C-98-richer-node-kinds.md).

## Progress
- 2026-07-30 — **The IR landed.** `crates/connector-spec/src/graph.rs`; `Connector::graphs`,
  `graphs_of`, `graph`. 18 tests in `tests/graphs.rs`, including a worked
  trigger → operation → gate → operation example. No artifact changed.
- 2026-07-30 — The design records the blocker that most threatens the idea: **`http.request` returns
  one flat string**, so a `Select` wired to an `Operation` output cannot lower until it returns a
  record. The IR may declare the port; C-95's lowering must refuse it.

## Notes
- **flux-lang has 43 node kinds; this repository constructs nine.** Every node here already exists in
  the language — `Throttle`, `Confirm`, `Retry`, `When`, `Jq`, `Fmt`, `Obj`, `Lit`. That is what makes
  this a projection rather than a language.
- **flux lifts only `op`.** A graph compiling to a whole Program has no artifact home; that is C-97's
  question, and the useful half ships without waiting for it.
- Do not merge with flux's Railflux (L-95): that projects an existing AST, this authors over
  catalogued members.
