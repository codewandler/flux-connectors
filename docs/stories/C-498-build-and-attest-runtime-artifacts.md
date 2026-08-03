---
id: C-498
title: "Build and attest connector runtime artifacts"
pillar: Build
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [runtime, release, supply-chain]
note: "a plugin/process/container connector needs an immutable binary or image identity and verified Exchange install path outside the Flux release pipeline"
---

# Build and attest connector runtime artifacts

## Goal

Give hand-written rich-protocol adapters a reproducible, signed connector artifact and Exchange
installation contract so vendor-specific Rust can leave Flux without weakening supply-chain
verification.

## Acceptance

- [ ] The connector bundle names protocol/API compatibility, platforms, entrypoint and immutable
      digests; tags, mutable paths and ambient `PATH` lookup are refused.
- [ ] Runtime adapter crates/images are built from this repository without entering the offline
      compiler dependency graph.
- [ ] The connector release path attests the artifact and Exchange verifies the same signed metadata
      and digest before first execution and after update; an unverifiable artifact never runs.
- [ ] The build/release path covers binaries and images without hand-run publication, and records
      provenance sufficient to trace a loaded artifact to reviewed source.
- [ ] No runtime artifact, index, signature or upload job enters the Flux release pipeline; Flux
      receives no artifact path and cannot execute one locally.
- [ ] A fixture proves tampering, platform mismatch and runtime-protocol mismatch each fail closed.

## Progress

- (not started)

## Notes

- Flux C-506 owns the disposition of the legacy `plugins/host-kit` and `plugins/pack-index` support
  crates; this story owns their connector-side replacement where needed.
