---
id: C-40
title: Ship provider icons as bundle assets
pillar: Build
status: done
design: docs/designs/connector-bundle.md
epic: connector-bundle
areas: [connector-cli, providers]
note: "CLOSED 2026-08-01 by C-437's answer, and refused as worded. No vendor mark is vendored: a brand guideline grants identification use to the DISPLAYER, revocably and non-sublicensably, while this repo's MIT/Apache-2.0 grants copy, modify and sublicense to everyone irrevocably — and git makes that unwithdrawable. The story's SHAPE survives: a mark ships beside the .flux, never base64 inside it"
---

# Ship provider icons as bundle assets

## Goal
Give each connector the branding a UI needs, without paying for it in every flux session.

## Acceptance
- [ ] Icons ship as **files** in the bundle directory, referenced by relative path from the manifest
      and the markdown page.
- [ ] **No icon is embedded in the `.flux` file.** A test asserts the module contains no base64 blob.
- [ ] At minimum an SVG per provider; rendered PNG sizes are optional and may come later.
- [ ] Icon paths are part of the manifest and are drift-checked like every other artifact.

## Progress
- (not started)

## Notes
- **Why not in the `.flux`.** `connectors/<name>.flux` is source that flux parses at session start
  (`DynamicComposites::load`). A base64 PNG is a few KB per size per format; across twenty providers
  that is hundreds of KB of base64 parsed on every session start, purely to reach the ops. It also
  makes the diff of a code artifact unreadable.
- **Blocked on a licensing answer, not a technical one.** Vendor logos are trademarked, and this repo
  is intended to be public. Settle whether we may ship Zendesk's and Freshdesk's marks before
  committing any. That question is why this story is `backlog` rather than `ready`.
- Rendering PNG sizes from an SVG needs a rasteriser — a build dependency the workspace does not
  have and that `connector-spec` must not take. If it happens at all it is `connector-cli`'s job.

## Progress
- **2026-08-01 — closed by [C-437](C-437-decide-the-logo.md), which refuses this story as worded.**
  This was filed noting it was *"blocked on a licensing answer, not a technical one"*. That answer is
  now written, and it is **no**: no vendor mark is vendored and no `logo_url` is declared.
- The licensing argument, because it is the whole reason: a brand guideline grants *identification
  use* to the party **displaying** the mark — revocably, non-transferably, non-sublicensably,
  conditioned on not modifying it. `LICENSE-MIT` and `LICENSE-APACHE` grant *copy, modify and
  sublicense*, perpetually, to everyone, over everything here. Vendoring puts those in direct
  contradiction over bytes this project does not own, and git history means a revocation could not be
  honoured even if we wanted to.
- **C-415's split does not transfer**, and that is the part worth keeping: an OpenAPI document is
  published *in order to be* implemented against, so scrubbing what must not travel makes the bytes
  publishable. A trademark exists *in order not to be* copied — there is nothing in the file to
  scrub, because the file is the problem.
- **What survives is the shape this story got right**: a mark ships *beside* the `.flux`, never
  base64 inside it. If the one exception C-437 leaves open is ever taken, that is the layout it uses.
