---
id: C-26
title: File the outbound $auth seam stories on flux's board
pillar: Bridge
status: blocked
design: docs/designs/auth-seam-flux-stories.md
epic: connectors-v1
areas: [flux-bridge]
note: **critical path** · 11 paste-ready drafts wait on a decision to write into ../flux
---

# File the outbound $auth seam stories on flux's board

## Goal
Get the `$auth` seam onto flux's backlog so it can ship in a flux release — the one external
dependency standing between generated connectors and a live API call.

## Acceptance
- [ ] The eleven drafts in [auth-seam-flux-stories.md](../designs/auth-seam-flux-stories.md) are
      filed on `../flux`'s board with flux's own ID scheme, or handed to a flux maintainer.
- [ ] Each filed story names its failing-first test (every draft already does, except the
      trust-model decision story, which is justified as having no behavioral change).
- [ ] Stories say **"outbound `$auth` header marker"** in their titles. flux already has a *done*
      `request-auth-seam` (D-64/D-68) for **inbound** bearer→principal resolution; without the
      qualifier reviewers will conflate the two.
- [ ] The flux release carrying the seam is recorded here, so C-15 knows what it is waiting for.

## Progress
- **Blocked on a decision, not on work.** The drafts are complete. Writing into
  `/home/timo/projects/flux` was deliberately out of scope for the C-16 run: that repo has the user's
  own uncommitted work, and filing stories into someone else's tree unasked is beyond what this repo
  should do on its own initiative.
- Status as of flux `v0.38.0`: **unscheduled**.

## Notes
- The drafts cover: the `$auth` marker itself; extracting flux's currently-private auth injection
  into a reusable pure function; deny-by-default credential resolution; redactor registration of the
  *composed* value; `http_hosts` scoping; `Query`-scheme injection; the `user_env` verbatim-resolution
  gap; and the trust-model decision.
- Two of them are worth flagging to flux regardless of this repo's schedule:
  - **A live redaction bug.** flux's plugin host composes a Basic credential
    (`../flux/crates/flux-plugin/src/host.rs:1253-1261`) and never registers it with the redactor.
    That is the shipped `plugins/zendesk` path, so a Basic credential can reach logs today.
  - **`AuthMethod.user_env` resolves verbatim**, so Zendesk's `<email>/token` suffix and Freshdesk's
    literal `X` password cannot be expressed. `Basic` is implemented-but-unusable for both.
