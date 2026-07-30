---
id: C-65
title: Prove inbound end to end on two vendors against a live flux
pillar: Build
status: backlog
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [providers, connector-cli]
note: "the epic's closing proof, mirroring C-15: two vendors chosen to exercise different halves — one spec-published webhook API with a simple scheme, one whose scheme carries a timestamp window"
---

# Prove inbound end to end on two vendors against a live flux

## Goal

Close the epic the way C-15 closed connectors-v1: a real vendor event, verified, routed, and handled by
a real flux — not a fixture.

## Acceptance

- [ ] Two vendors work inbound end to end. Pick them to exercise different halves: one with a
      spec-published webhook API and a plain body signature (GitHub), one whose scheme includes a
      timestamp window (Stripe or Slack).
- [ ] A real delivery from each vendor is verified and routed to a trigger, evidenced in the story.
- [ ] **Negative proof, recorded:** a tampered body and a stale timestamp each produce a rejection with
      **zero** deliveries — the fail-closed invariant, demonstrated rather than asserted.
- [ ] `flux-connectors check` detects an upstream event-schema change (invariant 7).
- [ ] The subscription op registers the endpoint, and unsubscribe removes it, leaving no orphan webhook.

## Progress
- (not started)

## Notes
- Depends on C-64's flux-side stories having landed in a flux release; sequence accordingly.
