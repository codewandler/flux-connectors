---
id: C-64
title: Design the flux-side verified-webhook seam and file its flux stories
pillar: Bridge
status: ready
priority: 4
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [bridge]
note: "the C-16 pattern repeated: flux's webhook channel has NO signature verification (bearer token only), so generated verification has nowhere to run — design the seam here, file the stories on flux's board, and let every other inbound story proceed without it"
---

# Design the flux-side verified-webhook seam and file its flux stories

## Goal

Specify what flux must gain for a verified, typed inbound event to be possible at all, and hand those
stories to flux's board early — this ships in a different repository on a different cadence, so it
blocks the finish, not the start.

## Acceptance

- [ ] A design section (in [inbound-events.md](../designs/inbound-events.md)) specifying the six
      flux-side capabilities: a declarative `verify` block on `channel webhook`; verification over the
      **raw body before parsing**; constant-time comparison plus timestamp tolerance; discriminator →
      trigger-label routing; a challenge/handshake hook answered without waking an agent; and the
      delivery id in the payload.
- [ ] A handoff artifact — [inbound-events-flux-stories.md](../designs/inbound-events-flux-stories.md)
      — carrying ready-to-paste flux stories, explicitly marked as **not** this board's backlog so
      `/track:board` never picks them up.
- [ ] Story ids in the handoff are marked **provisional** with the re-check command, because flux's
      fleet allocates ids concurrently (the C-16 handoff's claimed range was consumed by unrelated work
      before it was pasted — do not repeat that assumption).
- [ ] Every flux-side claim is anchored to a symbol, not a line number, and states the flux version it
      was verified against.

## Progress
- (not started)

## Notes
- Verified in flux: `crates/flux-channels/src/adapters/webhook.rs` has an optional static bearer token
  and **no** HMAC/signature path. `WebhookSettings` is `{ addr, path, async, token }`.
- Naming caution, exactly as C-16 hit: flux already has a **done** inbound `request-auth-seam`
  (bearer → principal). Call this one **webhook signature verification**, never "the inbound auth seam".
