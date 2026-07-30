---
id: C-131
title: "The IVR inbound event set, including the two different invites"
pillar: Spec
status: blocked
priority: 4
design: docs/designs/babelforce-ivr-atomics.md
epic: babelforce-ivr
areas: [providers, connector-spec]
note: "'on invite' is NOT the SIP INVITE of an inbound call — in this codebase it is the ACD inviting an AGENT to take a queued call (acd/handler.go:290-297). Both are real; they must not share a name"
---

# The IVR inbound event set, including the two different invites

## Goal

Declare babelforce IVR's reverse call direction as events, so a flow can react to a call arriving, an
agent being offered work, a recording finishing, and the rest.

## Acceptance

- [ ] The event set is declared through [C-58](C-58-inbound-events-epic.md)'s `EventDecl` — this
      story adds no new primitive.
- [ ] **The two invites are named apart, unmistakably.** In `internal/modules/acd/handler.go:290-297`,
      `q.callAgent(inviteCtx, agent)` is the ACD **inviting an agent** to take a queued call. That is a
      different event from the **inbound call** arriving at the platform. Both are worth exposing; one
      name for both would be a bug that reads as a feature.
- [ ] Each event declares its payload shape and, where the transport is a webhook, its **verification**
      — a webhook with no stated verification is already refused at load ([C-82](C-82-channel-bindings-epic.md)),
      and telephony events carry customer phone numbers, so this is not a formality.
- [ ] The event names are verified against what the platform actually emits, not inferred from module
      names. Say in Progress which you confirmed from source or docs and which you could not.
- [ ] No credential value in any declared event or verification block — the **credential is named**,
      never its value.
- [ ] The build stays a fixed point and the full gate is green.

## Notes

- **Depends on [C-130](C-130-ivr-atomics-inventory.md)** for the `ivr` service to exist. Events are
  members of a service, sharing one name namespace with its operations
  ([C-66](C-66-members-under-services.md)), so a collision between an event and an operation name is a
  load error — worth checking, since `dial` and `recording` are plausible names on both sides.
- Personal data: call events carry numbers and possibly recordings. Nothing in this repo stores them,
  but declaring a payload shape that includes them is a decision worth making explicitly rather than
  by transcription.
- If the platform's real event delivery turns out to be polling rather than webhooks, say so — a poll
  requires a cursor and is refused without one, which is a better outcome than a webhook declaration
  that no platform ever calls.

## Progress

- **Blocked before dispatch, on C-130's finding.** This story assumes events are cleanly per-atomic.
  [C-130](C-130-ivr-atomics-inventory.md) established from the Go source that **the atomics have no wire
  identity at all** — `parse_settings.go` maps *call-module* names onto them, and the `v2.*` identifiers
  appear in no wire document. If the atomics are not addressable outbound, "an event per atomic" needs
  the same source check before anyone writes TOML.
- **And the event set has not been located.** C-130 reports the only webhook document in that tree,
  `adapters/api/webhooksv1/openapi.yaml`, is a **single Ultimate.ai chat callback** — not an IVR event
  set. So the premise that there *is* a declarable IVR event set is itself unverified.
- The naming finding still stands and is still worth keeping: "on invite" in that codebase is the ACD
  inviting an **agent** to take a queued call (`internal/modules/acd/handler.go:290-297`), not the SIP
  INVITE of an inbound call. Whatever this story becomes, those must not share a name.
- **What would unblock it:** find where babelforce actually emits IVR events, from the source rather
  than from a document — and if it does not, this story closes with that recorded, exactly as C-130 did.

