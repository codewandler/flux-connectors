---
id: C-486
title: "Integrate and release the Asterisk ARI connector"
pillar: Build
status: ready
priority: 1
design: docs/designs/asterisk-ari-rest.md
epic: asterisk-ari-rest
areas: [providers, release, asterisk]
note: "coordinator-owned catalogue regeneration, full gates and immediate new-provider release"
---

# Integrate and release the Asterisk ARI connector

## Goal

Turn the scoped Asterisk implementation into a whole-catalogue fixed point and publish it immediately
as the newly added provider.

## Acceptance

- [ ] Full build regenerates every coordinator-owned catalogue artifact and the measured counts are
      recorded from commands in this session.
- [ ] Response coverage floor/absence ceiling changes, if required by their tests, are made only at
      integration.
- [ ] The complete Rust, public-site, host-page and publish-dry-run gates pass.
- [ ] Engineering and customer changelogs describe a spec-generated Asterisk ARI REST connector and
      do not claim event/WebSocket or Flux-plugin ownership.
- [ ] `scripts/cut-release.sh` cuts the required bump; main and tag are pushed, crates publication is
      watched green, and the GitHub release is created before the epic closes.

## Progress

- 2026-08-02: filed because the owner requires a release after each new provider.

## Notes

- This is coordinator-owned work; provider implementors do not regenerate whole-catalogue files.
