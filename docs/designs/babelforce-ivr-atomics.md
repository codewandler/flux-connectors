# Design: babelforce IVR v2 — atomics, not call modules

**Status:** proposed — **§The mismatch is a blocker, read it before scoping anything** ·
**Pillar:** Spec · **Stories:** [C-129](../stories/C-129-babelforce-ivr-epic.md) … C-132

> Read in `~/babelforce/projects/ivr/ivr` at the working tree present on 2026-07-30. Paths below are
> from that checkout.

## Why

babelforce's IVR v2 has two layers, and the connector has to choose which one it exposes.

**The module layer** — `internal/modules/`: `acd`, `agentic`, `audioplayer`, `dial`, `flow`, `read`,
`realtime`, `recording`, `switchnode`. These are the primitives: play audio, read a digit, branch,
dial, record, queue.

**The flow layer** — `flows/*.yaml`, where a node names a *call module* and carries `settings`:

```yaml
id: simple_menu_menu
module: simpleMenu
settings:
  prompts: [{ key: prompt, text: "Press 1 for Sales or 2 for Support" }]
  menuItems:
    - key: {name: KEY_1}
      application: {id: simple_menu_sales}
    - key: {name: KEY_2}
      application: {id: simple_menu_support}
  flowEndApplication: {id: simple_menu_no_result}
  readTimeout: 5
```

`simpleMenu` is not a primitive. It is `audioplayer` + `read` + `switchnode` welded together, with a
timeout. That is the whole observation behind this epic: **the call modules are compositions of the
atomics**, so exposing them as connector operations would publish seventeen frozen combinations
instead of six composable parts.

So the proposal — expose the atomics, and rebuild the call modules as templates over them — is the
right instinct. One thing makes it harder than it looks.

## The mismatch, which decides everything else

**A flow YAML is already a graph.** `flowEndApplication: {id}` is an edge. `menuItems[].application`
is a conditional edge. That is the same shape as [flow-graph.md](flow-graph.md)'s `Graph`, and the
resemblance is close enough to be tempting.

**But those edges are `goto`s**, and [C-94](../stories/C-94-flow-graph.md)'s model refuses exactly
that. Its rules are: no cycles, control flow must nest, no edge crosses a region boundary. Those
rules are not stylistic — they exist because **Flux has no `goto`**, so a graph that cannot be
topologically ordered cannot be lowered.

An IVR menu that re-prompts on invalid input jumps *backwards*. That is a cycle, and it is not an
edge case — it is what a menu **is**. `prompt_player_loop.yaml` is in the shipped flow set.

**A babelforce IVR flow is a state machine. C-94's graph is a nesting expression tree.** They are
different computational models, and treating one as the other is how this epic fails.

### The consequence people will miss: two different runtimes

A Flux composite op executes **in flux**. An IVR flow executes **in babelforce's own IVR engine** —
that is what `internal/ivr` and `internal/router` are. They are not the same runtime, and a graph
lowered for one does not run on the other.

`vision.md` already settles which side this repo is on: *"This repo compiles; flux executes."* An IVR
flow is a third thing — compiled here, executed by **the vendor**. Nothing in the current model
describes that, and pretending `Graph` covers it would produce artifacts that lower cleanly and run
nowhere.

## Approach

**1 · Expose the atomics as operations and events.** `provider=babelforce`, `service=ivr`,
`api_version=2` — the service level from [C-49](../stories/C-49-provider-services.md) is exactly the
right granularity, and babelforce already has `agent` and `call` surfaces that stay untouched.

The atomics are what a connector is for: each is one request or one event, with declared parameters
and declared risk. `audioplayer`, `read`, `switchnode`, `dial`, `recording`, `acd`.

**2 · Do not publish the call modules as operations.** `simpleMenu` and `promptPlayer` are
compositions; publishing them freezes a combination and hides its parts. This is the same rule
`vision.md` already states as a non-goal — *"Mechanically emitting all 400 endpoints of a large spec
produces an unusable tool catalog"* — applied one level up.

**3 · Events are the reverse direction, and one of them needs naming carefully.** "on invite" is
**not** the SIP INVITE of an inbound call. In this codebase `invite` is the ACD **inviting an agent**
to take a queued call — `internal/modules/acd/handler.go:290-297`, where `q.callAgent(inviteCtx,
agent)` runs per candidate agent. Both events are real and worth exposing; they must not share a
name. [C-58](../stories/C-58-inbound-events-epic.md)'s `EventDecl` already models this.

**4 · Templates are deferred behind a decision, not designed now.** Whether "template-like components
built from atomics" belong here depends on whether the graph gains an explicit state-machine profile
with bounded loops and vendor-side execution, or whether IVR composition simply stays in babelforce's
flow YAML. C-132 decides it. **Nothing in steps 1–3 depends on that answer**, which is why they go
first.

## On "we should probably just get rid of those"

Worth separating two claims, because only one of them is this repo's business:

- **In the connector: yes.** The call modules should not be operations. That is decided here.
- **In babelforce's backend: not this repo's call.** Whether `internal/modules/simpleMenu` should be
  deleted in favour of composed atomics is a change to a production telephony service with its own
  tests, flows in the field, and customers' numbers pointed at it. This design can say the atomics are
  sufficient to express it; it cannot say the migration is safe. If that is wanted, it is a story on
  babelforce's own backlog with a migration and a rollback, not a consequence of a connector epic.

## Out of scope

- **Editing anything under `~/babelforce`.** This repo describes; it does not refactor the vendor.
- **Lowering an IVR flow to Flux.** Blocked on C-132, and blocked *hard* on the goto/cycle mismatch.
- **`agentic` and `realtime` modules.** Both look like they carry streaming/model semantics that a
  request/response operation cannot express. Scope them only after the six plain atomics land.
