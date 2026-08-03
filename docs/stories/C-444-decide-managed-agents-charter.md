---
id: C-444
title: "Decide: may a connector create and run a Managed Agents session?"
pillar: Spec
status: ready
priority: 2
design: docs/designs/anthropic-managed-agents.md
epic: anthropic-managed-agents
areas: [connector-spec, providers]
note: "DECISION, not a task — the fourth shape of C-123's question. Management plane is ordinary SaaS; the SESSION plane runs an agent loop and bills inference, and flux has flux-agent/flux-orchestrate of its own. Nothing in the epic's session half may start before this"
---

# Decide: may a connector create and run a Managed Agents session?

## Goal

Answer one question in writing: **may a connector operation create a Managed Agents session and drive
it** — or is the session plane flux's, leaving this connector the management plane only?

This is a charter decision. It is filed so the option stays open and honest rather than being
half-built toward, exactly as [C-123](C-123-decide-connector-inference.md) is filed for inference and
[C-34](C-34-decide-proxy-charter.md) for the proxy. It produces a written answer, not code.

## Why it is not already answered by C-123

C-123 asks whether a connector may **serve** inference — whether `ai.*` may route through a
connector. Creating a Managed Agents session does not route `ai.*` anywhere; it asks Anthropic to run
an agent loop on its own orchestration layer. But it *does* cause inference to run and to be billed,
so the vision non-goal is adjacent rather than clearly clear. C-123's Progress already records the
question arriving in three shapes; **this is a fourth**, and it should be answered on its own terms.

## The two planes

| plane | surface | reading |
|---|---|---|
| **Management** | agents, environments, vaults, memory stores, skills, deployments — CRUD over configuration objects | Ordinary SaaS. In charter, same as `anthropic`'s existing `admin` service. |
| **Session** | create a session, send events, stream events | The decision. |

## The case against the session plane

- **flux already has an agent layer.** `../flux/crates/flux-agent`, `flux-orchestrate`, `flux-flow`.
  The argument that killed connector-served inference — *"a strictly worse second implementation of
  something that already works"* — has the same shape here.
- **It is protocol-rich and stateful.** A session holds a container and a long-lived SSE stream.
  C-495 says that still makes it a connector, but it requires the rich runtime and stream contracts;
  it is not evidence that one request/response operation can implement the session safely.
- **Cost.** A session bills inference and container time. A connector operation that spends money at
  an unbounded rate is a different risk class from a CRUD write, and `Risk` has no vocabulary for it.

## The case for, stated fairly

- **It is a paid SaaS HTTP API** from a vendor already in the catalogue, which is the charter's
  stated test.
- **The management plane is useless alone** to a flow that wants to *use* an agent.
- **The repository boundary is settled.** C-495/C-496 adopt `../flux/docs/designs/ecosystem.md`'s
  runtime axis: a connector describes an external capability and `plugin` is one runtime it may
  declare. This decision is only about duplicating an agent/inference plane, not where a rich
  integration belongs.
- **The channel binding already models the hard part.** Session creation is one request/one response;
  the stream is a `[[channels]]` declaration flux executes. Neither needs a runtime here.

## Acceptance

- [ ] A decision is recorded in [anthropic-managed-agents.md](../designs/anthropic-managed-agents.md)
      with its reasoning, and this story closes as `done` **whichever way it goes**. A "no" is a
      successful outcome.
- [ ] The decision names **who** it binds: this repository's charter, not flux's roadmap.
- [ ] If **management-plane only**: the epic is re-scoped to it and the reason is written into the
      design; the session operations and the socket binding are dropped from the epic, not deferred
      silently.
- [ ] If **yes**: `vision.md` is amended explicitly — *a non-goal quietly outgrown is worse than one
      changed on purpose* — and the cost/risk question above gets a follow-up story.
- [ ] It states whether it also answers C-123, or is independent of it.

## Progress
- (not started)

## Notes
- Do not start any session-plane implementation before this closes. That is the whole point of the
  story.
- The management half (C-445, C-446) is unaffected and may proceed in parallel.
