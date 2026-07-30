---
id: C-132
title: "Decide: do composed IVR templates belong here, and in what execution model?"
pillar: Spec
status: ready
priority: 4
design: docs/designs/babelforce-ivr-atomics.md
epic: babelforce-ivr
areas: [connector-spec, providers]
note: "DECISION — an IVR flow's edges are gotos and C-94's graph refuses cycles because Flux has none. And an IVR flow runs in the VENDOR's engine, a third case 'this repo compiles, flux executes' does not cover"
---

# Decide: do composed IVR templates belong here, and in what execution model?

## Goal

Answer, in writing, whether "template-like components built from atomics" belong in this repository —
and if so, what executes them.

This produces a decision, not code. It is filed so the question is answered deliberately rather than
discovered halfway through an implementation.

## Acceptance

- [ ] A decision is recorded in [babelforce-ivr-atomics.md](../designs/babelforce-ivr-atomics.md) with
      its reasoning, and this story closes `done` **whichever way it goes**. "No" is a successful
      outcome.
- [ ] The decision explicitly answers **what executes a template**: flux, babelforce's IVR engine, or
      nothing (it is documentation only). Leaving that unanswered is what makes this dangerous.
- [ ] If templates are **in**: state whether [C-94](C-94-flow-graph.md)'s `Graph` grows an explicit
      state-machine profile with bounded loops, or whether a separate model is needed. Amend the
      flow-graph design rather than quietly widening it, and file the follow-up stories.
- [ ] If templates are **out**: say so in the epic, and record that IVR composition stays in
      babelforce's flow YAML. [C-130](C-130-ivr-atomics-inventory.md) and
      [C-131](C-131-ivr-events.md) are unaffected either way.

## The two facts that force the question

**1 · The edges are `goto`s.** `flowEndApplication: {id}` and `menuItems[].application` jump to an
arbitrary application id. C-94's model refuses cycles, requires control flow to nest, and forbids an
edge crossing a region boundary — **because Flux has no `goto`**, so a graph without a topological
order cannot be lowered. A menu that re-prompts on invalid input jumps backwards, and
`prompt_player_loop.yaml` ships in babelforce's flow set. An IVR flow is a **state machine**; C-94's
graph is a **nesting expression tree**.

**2 · The runtime is the vendor's.** A Flux composite executes in flux. An IVR flow executes in
babelforce's IVR engine — `internal/ivr`, `internal/router`. `vision.md` says *"This repo compiles;
flux executes"*, and an IVR flow is a third case: compiled here, executed by **the vendor**. Nothing
in the current model describes that, and a graph lowered as though it were a Flux composite would
pass every gate and run nowhere.

## Notes

- **Do not start building templates before this closes.** That is the whole point, and it is the
  discipline [C-34](C-34-decide-proxy-charter.md) applies to the proxy and
  [C-123](C-123-decide-connector-inference.md) to inference.
- A tempting middle path — templates as pure documentation, generating nothing — is a legitimate
  answer and should be considered rather than dismissed as doing nothing. It captures the composition
  knowledge without claiming an execution model this repo does not have.
- Whether babelforce's backend should *drop* its call modules in favour of composed atomics is **not**
  this decision. That is a change to a production telephony service with live numbers pointed at it,
  and it belongs on babelforce's own backlog with a migration and a rollback.
