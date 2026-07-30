---
id: C-129
title: "babelforce IVR v2 — atomics, not call modules (epic)"
pillar: Spec
status: ready
priority: 3
design: docs/designs/babelforce-ivr-atomics.md
epic: babelforce-ivr
areas: [providers, connector-spec]
note: "EPIC — simpleMenu is audioplayer + read + switchnode welded together, so publishing call modules would freeze combinations instead of exposing parts. But an IVR flow's flowEndApplication is a GOTO, and C-94's graph refuses cycles because Flux has none"
---

# babelforce IVR v2 — atomics, not call modules (epic)

## Goal

Expose babelforce's IVR v2 as a connector service whose members are the **atomic** call primitives —
play, read, branch, dial, record, queue — rather than the composed call modules built on top of them.

## Acceptance

- [ ] `provider=babelforce`, `service=ivr`, `api_version=2`, using the existing service level from
      [C-49](C-49-provider-services.md). The `agent` and `call` surfaces are untouched.
- [ ] The atomics reach the catalogue as operations and events — [C-130](C-130-ivr-atomics-inventory.md).
- [ ] **No call module is published as an operation.** `simpleMenu` and `promptPlayer` are
      compositions; publishing them freezes a combination and hides its parts.
- [ ] The two distinct "invite" meanings are named apart — [C-131](C-131-ivr-events.md).
- [ ] Whether composed templates belong here at all is **decided in writing** before any are built —
      [C-132](C-132-decide-ivr-templates.md).

## Children

- [C-130](C-130-ivr-atomics-inventory.md) — the `ivr` service and its atomic inventory
- [C-131](C-131-ivr-events.md) — the inbound event set, including the two invites
- [C-132](C-132-decide-ivr-templates.md) — **decision**: state-machine profile, or vendor-side only?

## Notes

**The observation this epic rests on.** `internal/modules/` holds the primitives — `acd`, `agentic`,
`audioplayer`, `dial`, `flow`, `read`, `realtime`, `recording`, `switchnode`. `flows/*.yaml` composes
them under names like `simpleMenu`, which is `audioplayer` + `read` + `switchnode` plus a timeout. Six
composable parts beat seventeen frozen combinations, and that is the same reasoning `vision.md`
already applies to mechanically emitting every endpoint of a large spec.

**The blocker, and it is structural.** A flow YAML *is* a graph — `flowEndApplication: {id}` is an
edge, `menuItems[].application` a conditional one. But those are **`goto`s**, and
[C-94](C-94-flow-graph.md)'s model refuses cycles and requires nesting **because Flux has no `goto`**.
A menu that re-prompts on invalid input jumps backwards; that is not an edge case, it is what a menu
is, and `prompt_player_loop.yaml` ships in the flow set.

So a babelforce IVR flow is a **state machine** and C-94's graph is a **nesting expression tree**.
Different computational models.

**And two different runtimes.** A Flux composite executes in flux; an IVR flow executes in
babelforce's own IVR engine (`internal/ivr`, `internal/router`). `vision.md` says "this repo compiles;
flux executes" — an IVR flow is a third case, compiled here and executed by *the vendor*. Nothing in
the current model describes that, which is why C-132 exists.

**On "get rid of the call modules":** in the connector, yes — decided. In babelforce's backend, that
is not this repo's call. Deleting `simpleMenu` from a production telephony service with live numbers
pointed at it needs a migration and a rollback on babelforce's own backlog, not a consequence of a
connector epic.
