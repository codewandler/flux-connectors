---
id: C-85
title: The delivery envelope — flux's Event carries no id, source or verified flag
pillar: Bridge
status: ready
priority: 5
design: docs/designs/channel-bindings.md
epic: channel-bindings
areas: [bridge, flux]
note: "flux_app::Event is {label, payload} and nothing else, so 'delivery id in the payload' stuffs envelope into payload — and seed_payload binds every top-level payload field as a flow symbol, so a vendor key can silently shadow it"
---

# The delivery envelope — flux's Event carries no id, source or verified flag

## Goal
Name and close a gap the inbound design walked past: there is nowhere structured for a delivery's
*metadata* to live, so every scheme that needs it writes into the payload — where it collides with
the vendor's own fields.

## Acceptance
- [ ] The problem is stated with its two halves:
      1. `flux_app::Event` is `{ label, payload }` and nothing else — no id, no timestamp, no source,
         no "this was signature-verified" flag. So
         [inbound-events.md](../designs/inbound-events.md)'s "delivery id in the payload" is envelope
         data written into the message body.
      2. `flux_app`'s `seed_payload` binds the **whole payload to `$input` and every top-level field
         to its own symbol**. A vendor payload carrying a field named like an injected key silently
         shadows it, and a flow reading `{delivery_id}` cannot tell whose value it got.
- [ ] A decision: does flux's `Event` grow an envelope (id, received-at, source, verified), or does
      the binding declare a reserved prefix that seeding must not collide with? Record the trade-off
      — an envelope is the honest model and touches every channel; a prefix is cheap and leaves the
      shadowing possible for anyone who ignores it.
- [ ] **A flow can tell a verified event from an unverified one.** Today it cannot, and that is the
      part with a security edge: a program written against a signed webhook behaves identically if
      the operator later points an unverified transport at the same trigger label.
- [ ] Dedupe has somewhere to stand: the delivery id reaches a flow intact, so a redelivery is
      distinguishable from a second real event. Vendors redeliver, and delivery is at-least-once.
- [ ] Filed as flux stories in the C-84 handoff document, or in its own, with the same
      not-this-board's-backlog framing.

## Progress
- Not started. Found 2026-07-30 while reading flux's delivery path for
  [C-82](C-82-channel-bindings-epic.md).

## Notes
- Also worth carrying into the same handoff, found in the same read:
  - **Trigger matching is exact string equality** (`app.rs`), no globbing. Discriminator routing to
    `slack.app_mention` therefore needs one trigger declaration per event.
  - **`Bus::emit`'s run-routed branch drops on a full queue** and reports `0` for "dropped", "no
    listeners" and "channel gone" alike — flux's own story A-132, and it interacts with anything that
    depends on an inbound event actually arriving.
- This is the second cross-repo finding of the epic and, unlike C-84, it is **not** blocked on C-83:
  it is a fact about flux's delivery path that holds whether or not a connector describes the channel.
