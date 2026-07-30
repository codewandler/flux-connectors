---
id: C-40
title: Ship provider icons as bundle assets
pillar: Build
status: backlog
design: docs/designs/connector-bundle.md
epic: connector-bundle
areas: [connector-cli, providers]
note: alongside the .flux, never inside it
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
