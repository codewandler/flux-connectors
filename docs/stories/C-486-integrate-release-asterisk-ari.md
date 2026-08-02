---
id: C-486
title: "Integrate and release the Asterisk ARI connector"
pillar: Build
status: in-progress
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

- [x] Full build regenerates every coordinator-owned catalogue artifact and the measured counts are
      recorded from commands in this session.
- [x] Response coverage floor/absence ceiling changes, if required by their tests, are made only at
      integration.
- [x] The complete Rust, public-site, host-page and publish-dry-run gates pass.
- [x] Engineering and customer changelogs describe a spec-generated Asterisk ARI REST connector and
      do not claim event/WebSocket or Flux-plugin ownership.
- [ ] `scripts/cut-release.sh` cuts the required bump; main and tag are pushed, crates publication is
      watched green, and the GitHub release is created before the epic closes.

## Progress

- 2026-08-02: filed because the owner requires a release after each new provider.
- 2026-08-02: the full build measured 55 providers, 66 services, 841 connector operations and 1114
  artifacts; response coverage measured 715 of 841, with 126 honest response-shape absences.
- 2026-08-02: public-site build/tests passed 43 of 43 and the host page passed 15 of 15; the full
  Rust and publish-dry-run gates remain before the release is cut.
- 2026-08-02: `cargo test --workspace --no-fail-fast`, workspace build, Clippy with warnings denied
  and formatting all pass; publish dry-run is intentionally repeated after committing generated
  catalogue files because Cargo refuses to package a dirty published crate.
- 2026-08-02: clean-tree publish dry-run packaged and verified all four publishable crates without
  uploading anything.

## Notes

- This is coordinator-owned work; provider implementors do not regenerate whole-catalogue files.
