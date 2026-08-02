---
id: C-488
title: "Move the connector seam to Flux 0.52 for the flow-editor release train"
pillar: Build
status: done
priority: 0
epic: connectors-api
areas: [build, release]
note: "Flux 0.52.0 is published while connector-pack 0.15.0 still requires 0.49, so exchange cannot consume the current engine and pack through one runtime trait seam"
---

# Move the connector seam to Flux 0.52 for the flow-editor release train

## Goal
Prepare the published connector pack to share Flux 0.52 runtime types with flux-exchange, without
introducing a path/git dependency or a second engine line.

## Why now

Flux's flow-editor projection and execution trace were implemented in a local tree based on 0.51.1,
but registry and remote checks on 2026-08-02 show that Flux 0.52.0 is already published and remote
`main` has advanced beyond that tree. The
latest connector-pack still requires Flux 0.49, while the pack publicly exchanges `Tool`,
`ToolSpec`, `ToolContext`, and `flux_core::Result` with its host. Cargo treats those pre-1.0 minor
lines as incompatible types, so flux-exchange cannot adopt the editor release while it consumes the
published 0.15.0 pack.

This repository prepares the middle release in the required order: Flux publishes the editor
contract, flux-connectors publishes a pack on that engine line, then flux-exchange moves. Publishing
and tagging remain outside this implementation until explicitly requested.

## Acceptance
- [x] **Failing first:** changing only `ENGINE_LINE` to `0.52` makes
      `every_flux_requirement_states_the_recorded_line` name every manifest pin left on 0.49.
- [x] Every engine requirement moves together to `0.52`; `SPEC_LINE` remains independently pinned,
      and `Cargo.lock` contains one engine line.
- [x] The workspace compiles against crates.io dependencies only. No path or git dependency on
      `../flux` appears.
- [x] Generated artifacts remain a fixed point; if the engine changes their bytes, the change is
      reviewed and recorded rather than silently regenerated.
- [x] The full Rust gate is green. Node gates run only if their source or generated inputs change.
- [x] Engineering and customer changelogs state the compatibility break and downstream action.
- [x] The story records the remaining release-order blocker honestly: no tag, push, or publication
      occurs without an explicit request.

## Progress
- 2026-08-02 — Filed after verifying from the checked manifests that connector-pack 0.15.0 is on
  Flux 0.49 and the local editor work was based on Flux 0.51. The compatibility unit is the public runtime
  trait seam, not a source-level connector feature.
- 2026-08-02 — Corrected the target before changing the manifest: `cargo info` reports 0.52.0 as
  the latest Flux engine release, and `git ls-remote origin refs/heads/main` reports remote Flux main
  at `32ad580e` while this dirty local tree is at `41fc0777`. Moving only to 0.51 would knowingly
  leave the bridge one incompatible line behind.
- 2026-08-02 — The failing-first test named all six 0.49 requirements. The targeted Cargo update
  moved twelve resolved Flux packages, including the plugin protocol's transitive 1.2.0 → 2.0.0
  move, and all three engine-line tests pass with one 0.52 engine line and `flux-spec` still at 1.3.
- 2026-08-02 — Registry-source comparison found no `src/` difference in `flux-lang` or
  `flux-credentials`; core changed two source files, runtime and system five entries each, and web
  two. The workspace builds without source changes, and `connector-cli diff` reports `1114
  artifacts up to date (55 providers checked)`.
- 2026-08-02 — Full Rust gate passed: `cargo build --workspace`, `cargo test --workspace
  --no-fail-fast`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all
  --check`. Neither Node tree nor a generated input changed, so their suites were not run.
- 2026-08-02 — The four-crate packaging preflight was attempted with
  `scripts/publish-crates-io.sh --dry-run`; it stopped before packaging because Cargo refuses a
  dry-run from an uncommitted `Cargo.toml`. No `--allow-dirty`, commit, tag, push, or publication was
  performed. The preflight becomes runnable after an explicitly authorized commit.

## Notes
- Precedent: [C-455](C-455-move-the-flux-engine-line-to-049.md), the 0.47 → 0.49 move.
- The release order is Flux → wait for crates → flux-connectors → wait for its closure →
  flux-exchange. This implementation prepares the connector change; it does not publish it.
