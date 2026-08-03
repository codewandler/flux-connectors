---
id: C-498
title: "Build and attest connector runtime artifacts"
pillar: Build
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [runtime, release, supply-chain]
note: "a plugin/process/container connector needs an immutable binary or image identity, supported-platform contract and verified install path before either Flux or Exchange can run it"
---

# Build and attest connector runtime artifacts

## Goal

Give hand-written rich-protocol adapters a reproducible, signed connector artifact and installation
contract so vendor-specific Rust can leave Flux without weakening local or hosted supply-chain
verification.

## Acceptance

- [ ] The connector bundle names protocol/API compatibility, platforms, entrypoint and immutable
      digests; tags, mutable paths and ambient `PATH` lookup are refused.
- [ ] Runtime adapter crates/images are built from this repository without entering the offline
      compiler dependency graph.
- [ ] Flux and Exchange verify the same signed metadata and digest before first execution and after
      update; an unverifiable artifact never runs.
- [ ] The build/release path covers binaries and images without hand-run publication, and records
      provenance sufficient to trace a loaded artifact to reviewed source.
- [ ] A fixture proves tampering, platform mismatch and runtime-protocol mismatch each fail closed.

## Progress

- (not started)

## Notes

- Flux C-506 owns the disposition of the legacy `plugins/host-kit` and `plugins/pack-index` support
  crates; this story owns their connector-side replacement where needed.
