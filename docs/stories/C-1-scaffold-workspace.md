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
- [x] Root `Cargo.toml` declares a workspace with `crates/connector-spec`, `crates/connector-flux`,
      and `crates/connector-cli` (bin `flux-connectors`).
- [x] `connector-flux` depends on `codewandler-flux-lang` (lib `flux_lang`) as a git dependency
      pinned to a flux tag, and a smoke test proves the dependency resolves by parsing a trivial
      `.flux` source through `flux_lang::program::Module::parse_str`.
      **Pinned to crates.io `0.37`, not a git tag** — see the deviation in Progress below.
- [x] Workspace lints: `clippy` clean under `-D warnings`; `cargo fmt --all --check` clean.
- [x] Dual MIT/Apache-2.0 licence files, matching `../flux`.
- [x] `.gitignore` covers `target/` and local artifacts.
- [x] A CI workflow runs build, test, clippy `-D warnings`, and fmt check.

## Progress
- Workspace compiles and the full gate is green: `cargo build --workspace`,
  `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`.
- Added the two missing crate roots (`connector-flux/src/lib.rs`, `connector-cli/src/main.rs`) —
  without them the workspace manifest would not even load. Both are placeholders in the shape
  `connector-spec` already established: a module doc stating the crate's invariant, a `thiserror`
  error enum, and a `Result` alias. Real contents land with C-2 / C-8 / C-13.
- Smoke test: `crates/connector-flux/tests/flux_lang_smoke.rs::parses_trivial_flux_module` parses
  `flow ping\n  return null` through `flux_lang::program::Module::parse_str` and asserts the parsed
  module is a `Module::Flow` named `ping`. An integration test rather than a unit test on purpose —
  it exercises the dependency from a consumer's vantage, which is what "the pin resolves and its API
  is usable" actually means.
- **Deviation — the flux-lang dependency is a crates.io pin, not a git tag.** The Acceptance text
  above says "git dependency pinned to a flux tag"; that was written before the dependency was
  probed. `codewandler-flux-lang` is published, and `version = "0.37"` resolves to 0.37.0 cleanly.
  The two alternatives both fail outside a developer's own machine: the flux remote is reached via a
  custom SSH host alias (`git@codewandler:codewandler/flux.git`) that will not resolve in CI, and a
  `../flux` path dep is absent from a fresh clone. The rationale is recorded in a comment at the pin
  in the root `Cargo.toml`, and the now-wrong sentence in `AGENTS.md` ("as a git dependency pinned to
  a flux tag") has been corrected to say crates.io.
- `Cargo.lock` is committed — this is a workspace with a binary, so the lock is the reproducibility
  record. CI runs `cargo fetch --locked` so a stale lock fails by name instead of being silently
  repaired.
- Licence files are `../flux`'s verbatim, except the copyright line, which names "The
  flux-connectors authors" rather than "The flux authors".
- Not done here (deliberately, outside this story): no `providers/` or `specs/` directories yet, and
  no `flux-connectors` subcommands — those arrive with the stories that need them.

## Notes
- `connector-spec` must have **no network dependency** — that constraint is load-bearing for its
  testability and is stated in [AGENTS.md](../../AGENTS.md).
- flux tag to pin: `v0.37.0` was latest at scaffold time; confirm before pinning.
