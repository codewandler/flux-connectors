---
id: C-98
title: The node kinds flux-lang already has — parallel, match, each, saga, fallback
pillar: Spec
status: ready
priority: 6
design: docs/designs/flow-graph.md
epic: flow-graph
areas: [connector-spec, connector-flux]
note: "exposure, not invention — all of these are existing flux_lang::ast::Node variants this repo has never constructed. Match is what finally lets a value escape a conditional"
---

# The node kinds flux-lang already has — parallel, match, each, saga, fallback

## Goal
Expose the rest of flux's control vocabulary as nodes. `flux_lang::ast::Node` has 43 kinds; this
repository constructs nine, and the first wave of the graph adds a handful more.

## Acceptance
- [ ] **`Match` / `Route`** — multi-way branching with a default. This is the story that lets a value
      **escape a conditional**, which [C-94](C-94-flow-graph-epic.md) deliberately refuses: with a
      default arm, every path binds the region's output, so the symbol is always bound. Until then a
      gate exports nothing.
- [ ] **`Parallel`** — fan-out and join. The constraint is flux's: branches may not bind the same
      symbol, so two branches may not declare the same output port name, and the analyzer would reject
      it anyway. Also: no `return` inside a branch.
- [ ] **`Each`** — iterate a list, with `collect`. Bounded by construction.
- [ ] **`Saga`** — compensating steps, each with an `undo`, unwound in reverse on a later failure.
      The one node kind that changes what a *write*-heavy flow can promise.
- [ ] **`Fallback`** — ordered first-that-succeeds.
- [ ] **`Timeout`** and **`Budget`** as region kinds.
- [ ] Each lands with its lowering and a golden `.flux`, and each states which existing
      `flux_lang::ast::Node` it *is* — the claim that this is a projection has to stay checkable.

## Progress
- Not started.

## Notes
- Sequenced after [C-95](C-95-graph-lowering.md): the region machinery in the IR was built to take
  these additively, but there is no point declaring a kind nothing can lower.
- `Race` is deliberately absent from this list — its losing branches still count against an enclosing
  budget and still appear in the trace, which is a semantics worth deciding on rather than inheriting.
