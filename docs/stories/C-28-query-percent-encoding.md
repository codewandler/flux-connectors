---
id: C-28
title: Resolve percent-encoding for query values
pillar: Codegen
status: ready
priority: 6
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-flux, flux-bridge]
note: blocks zendesk ticket search · flux has no urlencode op at all
---

# Resolve percent-encoding for query values

## Goal
Decide and specify how a generated op percent-encodes query values, so a search expression containing
a space or `&` produces a correct request instead of a corrupted one.

## Acceptance
- [ ] The investigation is written up: can this be solved connector-side at all, or does flux need a
      new op? Answer with evidence from flux's registered-op catalog, not from assumption.
- [ ] If flux needs an op, a paste-ready story draft lands in
      [auth-seam-flux-stories.md](../designs/auth-seam-flux-stories.md)'s sibling handoff style,
      naming its failing-first test.
- [ ] The connector-side behaviour is specified: what the emitter does today, what it will do, and
      what it must **refuse** to emit rather than emit incorrectly.
- [ ] `zendesk.ticket.search` is shown either working or explicitly blocked with the reason.

## Progress
- (not started)

## Notes
- **Found by C-8, and it is not a small gap.** Query values are not percent-encoded, and flux has
  **no op that would do it** — the whole registered catalog was checked
  (`../flux/crates/flux-flow/docs/ops-reference.md`). A value containing a space, `&`, `#` or `=`
  corrupts the query string.
- Zendesk search expressions are exactly that shape (`type:ticket status:new`), and flux's own
  zendesk plugin percent-encodes them strictly for this reason
  (`provider-operation-inventory.md` §3.3.5). **So this blocks `zendesk.ticket.search`**, one of the
  seven operations the zendesk connector must cover to replace the plugin.
- C-8 deliberately did **not** half-fix it with `expr`'s `replace` for spaces only: that would look
  correct and be wrong, which is worse than an honest gap.
- Related flux-side gap, worth folding into the same investigation: flux has **no optional
  composite-op parameter** (`registry.rs:183-184` puts every param in `required_params`), so a model
  calling a six-filter op must pass six arguments to filter on one. Separate problem, same
  "flux needs a small change" bucket.
