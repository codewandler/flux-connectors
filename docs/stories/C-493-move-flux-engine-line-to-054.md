---
id: C-493
title: "Move the connector seam to Flux 0.54 for generated channels"
area: Build
status: done
priority: 1
areas: [build, release, channels]
note: "release-order bridge: Flux 0.54 publishes guarded WebSockets first, then connector-pack moves every engine pin together so flux-exchange links one runtime line"
---

# Move the connector seam to Flux 0.54 for generated channels

## Goal

Publish the generated channel plan on the same Flux runtime line as the guarded WebSocket system a
host uses to execute it, without introducing a second engine trait graph.

## Acceptance

- [x] Flux 0.54.0 is verified on crates.io before any connector dependency points at it.
- [x] **Failing first:** moving only one authored engine pin makes the engine-line manifest test fail.
- [x] All six authored Flux engine requirements move together from 0.52 to 0.54; the independent
      `flux-spec` line remains unchanged and `Cargo.lock` resolves exactly one engine line.
- [x] The complete four-crate publish closure packages and verifies with `cargo publish --dry-run`
      only; no local publish is attempted.
- [x] Full workspace, generated-artifact, site and host-page gates are green before the minor
      connector release is tagged and published through CI.

## Progress

- 2026-08-03: filed while Flux 0.54.0's exact-SHA binary release candidate is building; connector
  source remains on 0.52 until the registry artifacts are live.
- 2026-08-03: crates.io publication workflow 30781537591 completed successfully and registry search
  resolved the channel/server closure at 0.54.0 before the first manifest edit. Moving only
  `flux-lang` to 0.54 made `every_flux_requirement_states_the_recorded_line` fail with the exact
  0.54/0.52 mismatch; all six pins plus `ENGINE_LINE` then moved together, and all three manifest /
  lock / spec-line tests pass with one 0.54 engine line.
- 2026-08-03: the v0.17.0 cutter and CI gates completed; all four crates are visible on crates.io,
  and the site, host page, main CI and tagged publish workflows succeeded.
