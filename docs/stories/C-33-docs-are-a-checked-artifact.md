---
id: C-33
title: Treat generated docs as a checked artifact
pillar: Build
status: backlog
design: docs/designs/provider-docs.md
epic: provider-docs
areas: [connector-cli]
---

# Treat generated docs as a checked artifact

## Goal
Make a stale provider page a build failure, so generated documentation cannot quietly drift from the
connector it documents.

## Acceptance
- [ ] `flux-connectors check` fails when a committed page does not match what the current IR and
      generator would produce, naming the provider and what moved.
- [ ] `flux-connectors diff` previews page changes without writing.
- [ ] Page hashes are recorded alongside the other artifact hashes in `connectors.lock`, using the
      slot C-7 built — additively, not as a reshape.
- [ ] Building twice from unchanged inputs leaves pages byte-identical.

## Progress
- (not started)

## Notes
- Depends on C-31 and on C-7's lockfile, both of which already exist in the shape this needs.
- The whole argument of the repo is that drift should be *detected* rather than absorbed. Generated
  documentation is the artifact most likely to rot unnoticed, because nothing fails when it is wrong.
