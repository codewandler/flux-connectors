---
id: C-192
title: Move the flux pin from 0.39 to 0.41
pillar: Build
status: ready
priority: 1
design:
epic:
areas: [build]
note: "filed from ai-agent-platform 2026-07-31 — a downstream host must link ONE flux-runtime, and connector-pack hands it Arc<dyn Tool>. Two engine versions are two incompatible types"
---

# Move the flux pin from 0.39 to 0.41

## Goal

Track the published flux engine line so a host that consumes both flux and `connector-pack` can link
them, which today it cannot.

## Why now — a downstream host is blocked on it

`~/babelforce/projects/ai-agent-platform` is folding a Connectors service into its own image and
registering this repository's operations through `connector_pack::pack(…)`. That call hands
`Arc<dyn flux_runtime::Tool>` into the host's `ToolRegistry`. **Two `flux-runtime` versions are two
different types and will not link**, so the host, this repository and the vendored service must all
land on one engine line before any of it compiles.

State on 2026-07-31:

| Tree | Engine pin |
|---|---|
| this repository | **0.39** |
| ai-agent-platform | 0.24.1, with 0.36 in flight (its C-57/C-61) and 0.41.0 as the agreed target (its C-62) |
| crates.io `max_stable` | **0.41.0** (`codewandler-flux-*`); the local flux tree is at an unreleased 0.41.1 |

## Acceptance

- [ ] `flux-lang`, `flux-core`, `flux-runtime` and the `flux-system` dev-dependency move 0.39 → 0.41 in
      the workspace manifest.
- [ ] The protocol tier (`flux-spec`, its own 1.x line) is **re-checked against crates.io**, not
      assumed to stay at 1.2 — the manifest's own comment says it moves independently.
- [ ] `cargo run -p connector-cli -- diff` still reports every artifact up to date. Generated Flux is
      built as `flux_lang` AST and formatted by flux's formatter, so a formatter change upstream shows
      up here as artifact drift — if it does, regenerate and review the diff rather than pinning back.
- [ ] Gate green: `cargo build --workspace`, `cargo test --workspace`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [ ] `crates/connector-cli/tests/dependency_fence.rs` and `no_network.rs` still hold — the offline
      guarantee and the `connector-secrets` fence must not be a casualty of a version bump.
- [ ] Any `ToolSpec` / `Effect` / `Risk` / authority-layer changes between 0.39 and 0.41 are recorded
      here, because `connector-pack`'s `spec.rs` projection reads those types directly.

## Notes

- **Scout this before committing to a date.** Do the bump in a scratch worktree first and report
  whether the delta is mechanical; the downstream sequence is planned around the answer.
- flux 0.36 introduced an authority layer that validates every registered tool's contract
  (`ToolRegistry::register` panics on an invalid one). This repository is already past it at 0.39, but
  the downstream host is not — its C-61 characterised 39 failures. Worth knowing when reading their
  reports against ours.
- Prerequisite for [C-190](C-190-publish-catalog-pack-secrets.md); nothing downstream can consume the
  crates until they build against the host's engine version.
