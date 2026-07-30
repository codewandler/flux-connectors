---
id: C-1
title: Scaffold the Cargo workspace and the gate
pillar: Foundation
status: ready
priority: 1
design: docs/designs/connectors-v1.md
epic: connectors-v1
areas: [connector-spec, connector-flux, connector-cli]
note: everything else builds on this
---

# Scaffold the Cargo workspace and the gate

## Goal
Stand up the three-crate workspace, the flux-lang git pin, and a CI gate, so every later story lands
in a repo that already compiles and lints clean.

## Acceptance
- [ ] Root `Cargo.toml` declares a workspace with `crates/connector-spec`, `crates/connector-flux`,
      and `crates/connector-cli` (bin `flux-connectors`).
- [ ] `connector-flux` depends on `codewandler-flux-lang` (lib `flux_lang`) as a git dependency
      pinned to a flux tag, and a smoke test proves the dependency resolves by parsing a trivial
      `.flux` source through `flux_lang::program::Module::parse_str`.
- [ ] Workspace lints: `clippy` clean under `-D warnings`; `cargo fmt --all --check` clean.
- [ ] Dual MIT/Apache-2.0 licence files, matching `../flux`.
- [ ] `.gitignore` covers `target/` and local artifacts.
- [ ] A CI workflow runs build, test, clippy `-D warnings`, and fmt check.

## Progress
- (not started)

## Notes
- `connector-spec` must have **no network dependency** — that constraint is load-bearing for its
  testability and is stated in [AGENTS.md](../../AGENTS.md).
- flux tag to pin: `v0.37.0` was latest at scaffold time; confirm before pinning.
