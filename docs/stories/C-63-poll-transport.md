---
id: C-63
title: A `poll` transport — inbound for vendors with no webhook, no flux blocker
pillar: Codegen
status: backlog
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-flux]
note: "a cursor `op` (emitted) plus a documented `schedule`-channel program pattern (not emitted) — proves inbound is an abstraction over transports rather than a synonym for webhook, and ships with zero cross-repo dependency. AMENDED by C-82: the cursor is MANDATORY, because flux's cron drops ticks and replays none"
---

# A `poll` transport — inbound for vendors with no webhook, no flux blocker

> **Amendment ([C-82](C-82-channel-bindings-epic.md), [channel-bindings.md](../designs/channel-bindings.md)).**
> The cursor is **mandatory**, not optional, and the reason is a fact about flux rather than a
> preference: its schedule channel is one in-process task per channel, and **missed-tick replay is a
> named non-goal** of its own design (`../../flux/docs/designs/event-trigger-channels.md`). A restart
> drops ticks and replays none of them. So the schedule cannot be trusted to have run, and resuming
> from a recorded position is the only thing that makes a poll correct — a poll without a cursor loses
> events with nothing to detect it.
>
> `interval` is correspondingly **advisory**: the operator writes the actual schedule in their own
> program, this repository runs nothing, and the cadence is never a guarantee. The loader already
> enforces both rules (`crates/connector-spec/src/provider.rs`, `validate_channel_transport`), so what
> remains here is the emitted cursor op and the documented program pattern.

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
