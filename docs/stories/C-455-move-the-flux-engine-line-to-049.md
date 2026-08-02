---
id: C-455
title: "Move the connector seam to flux 0.49, then publish it before exchange moves"
pillar: Build
status: done
priority: 0
epic: connectors-api
areas: [build, release]
note: "owner-directed 2026-08-02: family releases are ordered flux, wait for its crates, flux-connectors, wait for its closure, then flux-exchange. flux 0.49.0 is live; connector-pack 0.12.0 still requires ^0.47 and is the one thing preventing exchange from moving"
---

# Move the connector seam to flux 0.49, then publish it before exchange moves

## Goal
Publish a connector-pack built against flux 0.49 so downstream hosts can move to the current engine
without resolving two incompatible `flux_runtime::Tool` traits.

## Why now

The owner directed the family release order explicitly: **flux → wait for its crates →
flux-connectors → wait for its closure → flux-exchange**. The first wait is complete: crates.io
answers `0.49.0`, not yanked, for `codewandler-flux-{core,lang,runtime,system,web}`. The newest
published `connector-pack` is 0.12.0 and still requires the 0.47 line, so it is the blocker rather
than a downstream source change.

This is breaking for `connector-pack` consumers even if its own Rust source does not change: the
public `Tool`, `ToolSpec`, `ToolContext` and `flux_core::Result` types move to a different pre-1.0
minor line. The connector release is therefore a minor bump, never a patch.

## Acceptance
- [x] **Failing first:** changing only `ENGINE_LINE` to `0.49` makes
      `every_flux_requirement_states_the_recorded_line` name every manifest pin left on 0.47.
- [x] Every engine requirement moves together to `0.49`; `SPEC_LINE` remains independently pinned,
      and `Cargo.lock` contains one engine line.
- [x] The workspace compiles against published crates only. No path or git dependency on `../flux`
      appears anywhere.
- [x] Generated artifacts are rebuilt and `connector-cli diff` reports a fixed point.
- [x] The full Rust gate, public-site gate, host-page gate and crates.io dry-run closure are green.
- [x] The engineering and customer changelogs state the compatibility break and the required
      downstream action.
- [x] A minor release is tagged and pushed through CI; all four published crates are visible on
      crates.io before flux-exchange is changed.

## Progress
- 2026-08-02 — Filed after verifying the five flux 0.49.0 crates above on crates.io and confirming
  `connector-pack 0.12.0` still requires `codewandler-flux-{core,lang,runtime} ^0.47`.
- 2026-08-02 — Failing-first guard named all six 0.47 manifest pins. The targeted Cargo update moved
  eleven resolved flux packages from 0.47.1 to 0.49.0 and no other package; all three engine-line
  tests pass and the full workspace builds.
- 2026-08-02 — Registry-source diff measured the boundary: core/web/credentials unchanged;
  runtime one additive constructor; lang canonicalization/CLI; system UDP/raw-ICMP dial variants.
  `connector-cli diff` reports `951 artifacts up to date (54 providers checked)`.
- 2026-08-02 — The full Rust gate is green with `--no-fail-fast`; the public site built and passed
  42 tests, the host UI passed 15, and the four-crate crates.io closure packaged and verified under
  `--dry-run` without uploading.
- 2026-08-02 — `v0.13.0` was cut at `ae8eaca`, pushed, and published by crates.io workflow
  `30730468297`, which confirmed all four crates. Registry search reports 0.13.0 for address, catalog,
  secrets and pack. CI workflow `30730467590` and Pages workflow `30730467582` are green, and the
  GitHub release is live.

## Notes
- Precedent: [C-431](C-431-move-the-flux-pin-to-0-47.md), the 0.46 → 0.47 engine move. The value is compatibility at the public trait
  seam; any behavioural difference discovered by the gate is recorded rather than assumed.
- C-454 integrated commit `6e421af` from the isolated worktree into `main`; its later interrupted
  release transaction is regenerated rather than copied.
