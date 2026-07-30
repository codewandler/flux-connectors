---
id: C-63
title: A `poll` transport — inbound for vendors with no webhook, no flux blocker
pillar: Codegen
status: backlog
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-flux]
note: "a cursor `op` (emitted) plus a documented `schedule`-channel program pattern (not emitted) — proves inbound is an abstraction over transports rather than a synonym for webhook, and ships with zero cross-repo dependency"
---

# A `poll` transport — inbound for vendors with no webhook, no flux blocker

## Goal

Give vendors without webhooks the same event surface as those with them, using only machinery that
already exists on both sides.

## Acceptance

- [ ] `transport = "poll"` with a cursor op and an interval emits (a) the **cursor `op`** into the
      module — an ordinary operation, which is legitimate — and (b) a **documented program pattern** for
      an operator's `channel schedule` + `trigger`. It does **not** emit a journey into the module
      (same constraint as C-61: flux lifts only `op` declarations).
- [ ] The consumer-visible surface is **identical** to the webhook case: a trigger label and a typed
      payload. Failing-first test `poll_and_webhook_present_the_same_event_surface`.
- [ ] The cursor is durable across runs, so a restart neither replays nor skips. The durable state
      lives on flux's side (the flow's store), not in a connector artifact — this repo ships no runtime.
- [ ] No new flux primitive is required — asserted by the story landing while C-64 is still open, which
      is the point of sequencing poll before the verified-webhook seam.

## Progress
- (not started)
