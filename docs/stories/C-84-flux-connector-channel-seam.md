---
id: C-84
title: Design the flux-side generic connector channel kind and file its flux stories
pillar: Bridge
status: done
design: docs/designs/connector-channel-seam.md
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
- [x] The design states how a program selects a connector-described surface, e.g.
      `channel support` / `kind "connector"` / `connector "slack"` / `binding "events-api"`, and where
      the manifest is read from. → [connector-channel-seam.md](../designs/connector-channel-seam.md)
      §"What the operator declares" and §"Where the manifest is read from"
      (`~/.flux/connectors/<connector>.connector.toml`, read through `flux_system::System`, mirroring
      `~/.flux/flows`).
- [x] **`build_channels` gains one arm, not one per vendor.** The acceptance is explicitly that
      `crates/flux-channels/src/adapters/slack.rs` can be deleted without losing behaviour: its
      payload map, its reply, its allow-lists and its bot/subtype loop guard each have a declared
      home, or the design says which do not and why. → §"Can `adapters/slack.rs` be deleted? An honest
      accounting" — a row-by-row table. **The answer is yes for behaviour, no for deployment shape**;
      two behaviours have *no* declared home and both gaps are on this side, not flux's.
- [x] The **reply path is an operation call**, not new adapter code. → §"The reply is an operation
      call — this is the crux", filed as flux `D-217` + `D-218`. Note the story's premise needed
      correcting: a stored `~/.flux/flows` op is reachable only via the `flow_run` tool and cannot make
      a live vendor call at all until the `$auth` seam ships, so the reply targets the **Tool pack**'s
      registered operation ([connector-tool-pack.md](../designs/connector-tool-pack.md)) and `flow_run`
      is recorded as the fallback.
- [x] The reply traverses the normal authorization → approval → guarded-IO envelope. → `Executor::dispatch`,
      via a defaulted `Deliverer::call_operation`, under an allow-list of **exactly one op** (flux `D-217`).
- [x] **Allow-lists stay operator config, not connector data.** → §"Allow-lists are operator config,
      deliberately", filed as flux `D-219`. Generalised without new IR: the keys are payload symbols the
      binding declares, so a typo is a load error rather than a filter that silently allows everyone.
- [x] Verification runs in flux over the **raw request bytes, before parsing**, with constant-time
      comparison and the declared tolerance — consuming C-60's matrix rather than restating it.
      → consumed, not restated: flux `C-291`/`C-292` were filed from [C-64](C-64-flux-webhook-seam.md)
      while this design was being written, and `D-215` depends on them. §"Verification: raw bytes,
      before parsing" records the one thing this seam adds — the parameters come from the manifest
      rather than from a hand-written program block.
- [x] The discriminator routes to a **fully-qualified trigger label**. → §"Routing". **Decided: no
      prefix matching**, N events means N triggers, and the verbosity is a generable snippet rather than
      a language change. flux `C-294` owns the general mechanism; `D-216` narrows it — a discriminator
      value outside the binding's closed `events` set is a logged no-op, never a label, because
      otherwise a vendor names this host's trigger labels.
- [x] Filed as a handoff artifact (`docs/designs/channel-bindings-flux-stories.md`), explicitly **not
      this board's backlog**. → written as a **ledger** rather than paste blocks, because the stories are
      actually filed this time.

## Progress
- 2026-07-30 — **Design written**: [connector-channel-seam.md](../designs/connector-channel-seam.md).
  Every `path:line` verified in `/home/timo/projects/flux` at workspace 0.40.0, commit `2abd0a13`.
- 2026-07-30 — **Six stories filed on flux's board**, uncommitted in `/home/timo/projects/flux/docs/stories/`,
  epic `connector-channels`, pillar `Agent`: `D-215` (epic), `D-216` (the arm + every load-time refusal),
  `D-217` (the dispatch seam), `D-218` (the reply through the Tool pack, deleting the hand-built
  `chat.postMessage`), `D-219` (allow-lists), `D-220` (Socket Mode as a transport). Ledger:
  [channel-bindings-flux-stories.md](../designs/channel-bindings-flux-stories.md). flux's board file
  was **not** touched.
- 2026-07-30 — **Scope shrank mid-story, correctly.** flux's `C-291`…`C-295` (epic
  `verified-webhook-channel`) were filed from C-64 while this was in flight. The plan had been to file
  the raw-body/HMAC primitive here; it is now a dependency instead. `D-215` states the division: one
  verifier, two declaration sources.
- 2026-07-30 — **Four findings recorded for this repository** (§"What this repository owes"). Each
  deserves its own story: absence matching in `EventDecl::when`; `providers/slack.toml`'s loop guard
  living in `schema` rather than `when`; no `[channels.challenge]` on `ChannelBinding`; and the loader
  not refusing a body-sourced verification timestamp.

## Notes
- **Two facts in the original Notes were stale and are corrected in the design.** `AppDeliverer` does
  *not* serialize deliveries behind a mutex — deliveries are concurrent and bounded (flux A-112/A-129,
  `crates/flux-channels/src/lib.rs:19-38`). And `adapters/slack.rs` is 217 lines, not ~218.
- Still true, re-verified: `build_channels`' `match` is closed and an unknown kind is a hard load error;
  the `webhook` channel has an optional static bearer and no signature path; `send` records but prints
  only for a `cli` channel, so a journey cannot reply through it.
- **The two blocking gaps are ours, not flux's.** Slack's `message` event stays unusable until
  `EventDecl::when` can express absence (the loop guard), and no webhook-transport binding is
  registerable until `ChannelBinding` can declare a challenge. That is why flux `D-220` ports Socket
  Mode instead of treating it as optional.
- Depends on [C-83](C-83-channel-binding-codegen.md): flux cannot read a binding that no manifest
  publishes. The reply additionally depends on [C-115](C-115-request-delegation.md)/[C-117](C-117-pack-codegen.md).
