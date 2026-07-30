---
id: C-84
title: Design the flux-side generic connector channel kind and file its flux stories
pillar: Bridge
status: ready
priority: 4
design: docs/designs/channel-bindings.md
epic: channel-bindings
areas: [bridge, flux]
note: "this is what retires adapters/slack.rs — one generic `connector` arm in build_channels instead of one arm per vendor. Cross-repo handoff, per the C-16/C-64 precedent"
---

# Design the flux-side generic connector channel kind and file its flux stories

## Goal
Specify the flux change that makes a connector's channel binding *executable*, and file it on flux's
board as a ready-to-paste handoff — the precedent [C-16](C-16-design-auth-seam.md) set for the
outbound `$auth` seam and [C-64](C-64-flux-webhook-seam.md) repeats for verified webhooks.

## Acceptance
- [ ] The design states how a program selects a connector-described surface, e.g.
      `channel support` / `kind "connector"` / `connector "slack"` / `binding "events-api"`, and where
      the manifest is read from.
- [ ] **`build_channels` gains one arm, not one per vendor.** The acceptance is explicitly that
      `crates/flux-channels/src/adapters/slack.rs` can be deleted without losing behaviour: its
      payload map, its reply, its allow-lists and its bot/subtype loop guard each have a declared
      home, or the design says which do not and why.
- [ ] The **reply path is an operation call**, not new adapter code — flux already loads the
      connector's module from `~/.flux/flows`, so answering an event is invoking an `op` it already
      has. This is the crux: if flux cannot make that call through its own executor, the binding
      buys nothing.
- [ ] The reply traverses the normal authorization → approval → guarded-IO envelope. A channel that
      could post to Slack outside the executor would be a second, unpoliced request path.
- [ ] **Allow-lists stay operator config, not connector data.** `allow_users`/`allow_channels` are a
      deployment policy about *who may trigger this agent*, and a vendor spec cannot know them. The
      design must place them on the flux side deliberately rather than by omission.
- [ ] Verification runs in flux over the **raw request bytes, before parsing**, with constant-time
      comparison and the declared tolerance — consuming C-60's matrix rather than restating it.
- [ ] The discriminator routes to a **fully-qualified trigger label**, so `trigger on
      "slack.app_mention"` is possible. Note that flux's trigger matching is exact string equality
      with no globbing, so N declared events means N trigger declarations unless flux gains prefix
      matching — decide and record which.
- [ ] Filed as a handoff artifact (`docs/designs/channel-bindings-flux-stories.md`), explicitly **not
      this board's backlog**.

## Progress
- Not started.

## Notes
- **Verified facts about flux as of 2026-07-30**, worth re-checking before filing:
  - `crates/flux-channels/src/adapters/mod.rs` dispatches `kind` through a closed `match`; an unknown
    kind is a hard load error, so a plugin cannot supply a channel kind either.
  - `adapters/slack.rs` is ~218 lines; `on_push` ends by calling `chat_post_message` directly.
  - The `webhook` channel authenticates with an optional static bearer token and has **no signature
    path at all** — the blocker C-64 owns.
  - `send`/`ask` only actually emit for the `cli` channel; every other kind replies via the
    `Vec<JourneyRun>` returned from `deliver`.
- Depends on [C-83](C-83-channel-binding-codegen.md): flux cannot read a binding that no manifest
  publishes.
