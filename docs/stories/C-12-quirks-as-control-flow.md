---
id: C-12
title: Compile quirks into Flux control flow
pillar: Codegen
status: backlog
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-flux]
note: the payoff for targeting a real language
---

# Compile quirks into Flux control flow

> **Amendment ([C-94](C-94-flow-graph-epic.md), [flow-graph.md](../designs/flow-graph.md)).** This
> story and [C-95](C-95-graph-lowering.md) need **the same construction**. C-12 turns a declared
> `RateLimit` into a `throttle` and a declared `Pagination` into a bounded loop; a graph's `throttle`
> and `retry` region nodes lower to the identical `flux_lang::ast` shapes.
>
> They must share one lowering. Two code paths emitting different Flux for the same intent — a
> differently-derived bucket name, a different backoff default — is the failure to avoid, and it would
> surface as two providers behaving differently for reasons nobody could see. Whichever story lands
> first owns the helper; the second consumes it.

## Goal
Turn declared quirks — retries, rate limits, pagination — into real Flux control flow, delivering the
capability that a config-interpreting integration layer could never have.

## Acceptance
- [ ] Transient-failure quirks emit `retry <n> backoff exponential delay <ms>` around the request.
- [ ] A declared rate limit emits `throttle` with a stable, per-connector unique bucket name.
- [ ] Page-based and cursor-based pagination emit a **bounded** loop that accumulates results, with
      an explicit page cap.
- [ ] Golden-file tests for each quirk kind.
- [ ] Quirks are opt-in per operation from the provider TOML; an operation with no quirks emits a
      plain request.

## Progress
- (not started)

## Notes
- flux's analyzer rejects unbounded loops, so pagination must carry an explicit `max` — a constraint
  worth honoring rather than working around.
- `throttle` bucket names must be unique within a session or buckets collide
  (`../flux/crates/flux-lang/docs/reference.md`, `throttle` node).
- `saga` for multi-step compensating writes is a natural extension but is **not** in this story —
  keep it for after milestone 1.
