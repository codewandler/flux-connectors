---
id: C-446
title: "Can HmacSpec express the Managed Agents webhook signature, and what is the event set?"
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/managed-agents-verification.md
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

- [x] The vendor's webhook signature scheme is written down precisely (headers, signed content,
      digest, encoding, freshness window), sourced from the bundled `claude-api` skill reference —
      invoke the skill rather than answering from memory.
      → [managed-agents-verification.md](../designs/managed-agents-verification.md) § *What the vendor
      publishes*, from `shared/managed-agents-webhooks.md` read 2026-08-02. **Two of the five are
      published and three are not:** the three headers and the ~5-minute window are stated; the signed
      string, the digest and the encoding are **not**, because the reference directs callers to
      `client.beta.webhooks.unwrap()` instead. Recorded as unknowns rather than filled from a
      convention — see § *What this repository must not do about the missing three*.
- [x] A **verdict**: expressible in `HmacSpec` today, or not. If not, the missing axis is named
      exactly, in the shape C-141/C-188 used, and a follow-up story is filed.
      → **not expressible**, and the design separates two gaps. **Gap B**, unconditional and the actual
      blocker, is filed as [C-447](C-447-verification-scheme-is-unmodelled.md): `VerificationScheme`
      has no state for *"the vendor signs, and we cannot model how"*. **Gap A** — `signed` cannot
      interpolate a selected per-delivery value such as `webhook-id` — is C-141/C-188's class and is
      **deliberately not filed**, because whether `webhook-id` is signed is unverified.
- [x] The **stream** event vocabulary is enumerated (`agent.message`, `agent.tool_use`,
      `agent.custom_tool_use`, `session.status_idle`, `session.error`, `span.model_request_end`, …)
      and the **webhook** `data.type` vocabulary separately. The two are different namespaces and the
      story says so.
      → design § *The two event vocabularies*, two tables and a § on how different they are.
      `session.status_idle` (stream) vs `session.status_idled` (webhook) is the near-collision that
      makes the separation worth stating; `session.thread_created` is the one name in both sets.
- [x] It is confirmed that **two bindings over two different event sets on one service** is
      expressible — `slack` proved two bindings, but over *one* event set. If the model cannot carry
      two vocabularies, that is a finding.
      → **expressible, no model change needed**, confirmed by reading `validate_channel_events`
      (`crates/connector-spec/src/provider.rs:3011`): it demands only that each event is declared and
      shares the binding's service. No rule couples two bindings' event sets.
- [ ] The **socket binding declares no verification**, and the reason is written in the provider file
      the way `providers/slack.toml:450` writes it: nothing arrives unsolicited over a socket we
      opened and authenticated.
      → **not done, and could not be:** `providers/anthropic.toml` is fenced for this story (C-441
      holds it) and the socket half is gated on C-444, still `ready`. The design settles the rule and
      the wording, and records that stating *any* verification on a `socket` binding is a **loader
      error** (`provider.rs:3047`) — stronger than "needs none".
- [ ] The socket binding's `reply` names `POST /v1/sessions/{id}/events` as a **declared operation of
      the same connector** — not a parallel reply mechanism.
      → **not done, same two reasons.** Settled as a design rule, with an **open question flagged**:
      the documented SSE frames carry no session identifier, so `session_id` may not be bindable from
      the `payload` map. Must be resolved against a full event schema before the declaration story is
      dispatched.
- [x] **Failing-first test** if anything lands in the loader: extend
      `crates/connector-spec/tests/verification_conformance.rs`, which is where C-60 put the real
      vendor vectors.
      → **nothing landed in the loader.** The story's answer is a negative result and a filed gap, so
      this is vacuous; C-447 carries the failing-first requirement instead.
- [x] Gated on [C-444](C-444-decide-managed-agents-charter.md) for the **socket** half only — the
      webhook half is management-plane and may proceed regardless.
      → respected. C-444 is still `ready`, so the socket half is answered *on paper only* and nothing
      is declared. The webhook half proceeded and produced the verdict.

## Progress

**Answered, as a negative result, in
[docs/designs/managed-agents-verification.md](../designs/managed-agents-verification.md). One gap
filed: [C-447](C-447-verification-scheme-is-unmodelled.md). Docs-only — nothing landed in the loader,
so there is no failing-first test and none is owed.**

- **The premise moved, and the story is better for it.** The story predicted `HmacSpec` would be one
  axis short because a *delivery id* is signed. That may be true and **cannot be established**: the
  bundled reference does not publish the signed string at all. The blocking gap is one layer up —
  `VerificationScheme` cannot express *"the vendor signs, and we cannot model how"*, so the only
  declarations that load today are a guess or a lie. That is C-447, and it is a different class from
  C-141/C-188 (which widened `HmacSpec` because a vendor's *parameters* did not fit).
- **`verification = "none"` was the tempting wrong answer and is refused with its reason written
  down.** It would be a **false** statement, not a conservative one — the vendor signs every delivery,
  and a manifest saying otherwise tells a host there is nothing to check.
- **Four of nine `HmacSpec` fields are fillable** — `header`, `secret`, `timestamp`, `tolerance`.
  `algorithm`, `encoding`, `prefix`, `signed` and `timestamp_format` are not; the last of those is the
  quiet one, because it is `Option` and absent means `unix_seconds`, so omitting it **guesses** rather
  than refusing. (This bullet said "five of nine" on the first pass and was wrong — corrected against
  `sed -n '152,237p' crates/connector-spec/src/inbound.rs | grep -c "^    pub "` → `9`.)
- **The interim state is Twilio's between C-109 and C-188:** declare the events, declare no
  `[[channels]]` binding for the webhook half. C-447's own § *Why this is worth a state rather than an
  omission* records what that costs.
- **Two loader facts were read rather than assumed**, and one contradicted the epic design:
  - two bindings over **disjoint** event vocabularies on one service needs **no model change**
    (`validate_channel_events`, `provider.rs:3011`);
  - declaring *any* verification on a `socket` binding is a **loader error**, not merely unnecessary
    (`validate_channel_verification`, `provider.rs:3047`). The parent design says the socket binding
    "needs no verification"; the model is stricter than that.
- **An open question is flagged for the declaration story, not resolved here.** `SendEvents` requires a
  `session_id` path parameter and the documented SSE frames carry no session identifier, so the
  socket binding's reply may be unbindable — a third gap of a different kind (a value constant for the
  *connection*, not present in the *event*). The reference shows example frames, not a schema, so this
  is an open question and an implementor must not be sent at it before it is settled.
- **Board not regenerated.** `docs/stories/README.md` is coordinator-owned; C-447's row and this
  story's status change need `/track:board` at integration.

## Notes
- Do not edit `providers/anthropic.toml` while C-441 is unintegrated.
- `EventDecl` names admit `-`, `_` and `.`, so the vendor's own spelling (`session.status_idle`)
  survives into the member namespace unchanged.
