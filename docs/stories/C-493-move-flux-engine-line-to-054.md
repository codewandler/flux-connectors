---
id: C-493
title: "Move the connector seam to Flux 0.54 for generated channels"
area: Build
status: in-progress
priority: 1
areas: [build, release, channels]
note: "release-order bridge: Flux 0.54 publishes guarded WebSockets first, then connector-pack moves every engine pin together so flux-exchange links one runtime line"
---

# Move the connector seam to Flux 0.54 for generated channels

## Goal

Publish the generated channel plan on the same Flux runtime line as the guarded WebSocket system a
host uses to execute it, without introducing a second engine trait graph.

## Acceptance

- [ ] Flux 0.54.0 is verified on crates.io before any connector dependency points at it.
- [ ] **Failing first:** moving only one authored engine pin makes the engine-line manifest test fail.
- [ ] All six authored Flux engine requirements move together from 0.52 to 0.54; the independent
      `flux-spec` line remains unchanged and `Cargo.lock` resolves exactly one engine line.
- [ ] The complete four-crate publish closure packages and verifies with `cargo publish --dry-run`
      only; no local publish is attempted.
- [ ] Full workspace, generated-artifact, site and host-page gates are green before the minor
      connector release is tagged and published through CI.

## Progress

- 2026-08-03: filed while Flux 0.54.0's exact-SHA binary release candidate is building; connector
  source remains on 0.52 until the registry artifacts are live.
