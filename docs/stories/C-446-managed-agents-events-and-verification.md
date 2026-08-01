---
id: C-446
title: "Can HmacSpec express the Managed Agents webhook signature, and what is the event set?"
pillar: Spec
status: ready
priority: 2
design: docs/designs/anthropic-managed-agents.md
epic: anthropic-managed-agents
areas: [connector-spec, providers]
note: "the conformance half. Three headers (webhook-id/-timestamp/-signature) against an HmacSpec that models ONE signature header — C-141 and C-188 are the precedents for finding it an axis short. A negative result is a successful outcome"
---

# Can HmacSpec express the Managed Agents webhook signature, and what is the event set?

## Goal

Two linked questions, answered against the model rather than around it:

1. **Is the Managed Agents webhook signature expressible in `HmacSpec` as it stands?**
2. **What are the two event vocabularies**, and do they fit `EventDecl` and two `ChannelBinding`s on
   one service?

## Why this is the risky half

`HmacSpec` (`crates/connector-spec/src/inbound.rs:152`) models a signature as: one `header` carrying
the digest, an `algorithm`, an `encoding`, an optional literal `prefix`, and a `signed` template over
a **closed** placeholder set — `{body}`, `{timestamp}`, `{sorted_form}`, `{url}` — with `tolerance`
required exactly when `{timestamp}` is present (`:561`).

The vendor's scheme uses **three headers** — `webhook-id`, `webhook-timestamp`, `webhook-signature` —
with a freshness window of roughly five minutes. A signature whose signed content includes a
**delivery id** has no placeholder in that closed set.

**Both outcomes are successful outcomes.** If it is expressible, declare it. If it is not, the output
is a precise gap filed against `HmacSpec` — which is exactly what
[C-141](C-141-hmac-spec-gaps.md) and [C-188](C-188-hmac-form-signing.md) both were. What is **not**
acceptable is inventing a scheme, half-declaring one, or setting `verification = "none"` to make the
loader accept a webhook whose signature we could not model: `AGENTS.md` — *"Silence is never a
verification answer"*, and *"Never present an unverified event as trusted."*

## Acceptance

- [ ] The vendor's webhook signature scheme is written down precisely (headers, signed content,
      digest, encoding, freshness window), sourced from the bundled `claude-api` skill reference —
      invoke the skill rather than answering from memory.
- [ ] A **verdict**: expressible in `HmacSpec` today, or not. If not, the missing axis is named
      exactly, in the shape C-141/C-188 used, and a follow-up story is filed.
- [ ] The **stream** event vocabulary is enumerated (`agent.message`, `agent.tool_use`,
      `agent.custom_tool_use`, `session.status_idle`, `session.error`, `span.model_request_end`, …)
      and the **webhook** `data.type` vocabulary separately. The two are different namespaces and the
      story says so.
- [ ] It is confirmed that **two bindings over two different event sets on one service** is
      expressible — `slack` proved two bindings, but over *one* event set. If the model cannot carry
      two vocabularies, that is a finding.
- [ ] The **socket binding declares no verification**, and the reason is written in the provider file
      the way `providers/slack.toml:450` writes it: nothing arrives unsolicited over a socket we
      opened and authenticated.
- [ ] The socket binding's `reply` names `POST /v1/sessions/{id}/events` as a **declared operation of
      the same connector** — not a parallel reply mechanism.
- [ ] **Failing-first test** if anything lands in the loader: extend
      `crates/connector-spec/tests/verification_conformance.rs`, which is where C-60 put the real
      vendor vectors.
- [ ] Gated on [C-444](C-444-decide-managed-agents-charter.md) for the **socket** half only — the
      webhook half is management-plane and may proceed regardless.

## Progress
- (not started)

## Notes
- Do not edit `providers/anthropic.toml` while C-441 is unintegrated.
- `EventDecl` names admit `-`, `_` and `.`, so the vendor's own spelling (`session.status_idle`)
  survives into the member namespace unchanged.
