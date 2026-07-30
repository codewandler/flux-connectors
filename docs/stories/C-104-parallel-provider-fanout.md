---
id: C-104
title: Make whole-catalogue artifacts coordinator-owned, so provider stories can run in parallel
pillar: Build
status: ready
priority: 2
design:
epic: provider-fleet-2
areas: [connector-cli, catalog]
note: "the fan-out cap is ONE file — crates/catalog/src/generated.rs carries two hand-maintained lists every provider story appends to, so any two collide and the wave size is 1"
---

# Make whole-catalogue artifacts coordinator-owned, so provider stories can run in parallel

## Goal
Let N provider stories run at once. Today any two collide, so the fleet ships one connector at a
time no matter how many implementors are available.

## The cap, precisely

A provider story writes `providers/<id>.toml` and, through the build, these artifacts:

| artifact | shape | collides? |
|---|---|---|
| `connectors/<id>.flux`, `<id>.connector.toml` | per-provider | no |
| `crates/catalog/ops/<id>/*.flux`, `src/generated/<id>.rs` | per-provider | no |
| `crates/connector-flux/tests/<id>_connector.rs` | per-provider | no |
| **`crates/catalog/src/generated.rs`** | **two hand-maintained lists** | **always** |
| **`web/public/catalog.json`** | whole-catalogue, full-build only | **always** |

`generated.rs` is the binding constraint. It carries a `pub(crate) mod <id>;` line *and* a
`&<id>::PROVIDER` entry in `PROVIDERS`, both appended by hand. Its own doc comment explains why —
*"`build --provider zendesk` compiles a single provider, so an index generated from that run would
have to drop the other two"* — which is sound, and is exactly what caps the fan-out at one.

`catalog.json` already solves the same problem the right way: it is **emitted only on a full run**,
and a scoped build leaves the committed document untouched rather than truncating it.

## Acceptance
- [ ] `crates/catalog/src/generated.rs` is **generated on a full build** and leaves untouched by a
      `--provider` or `--service` run — the rule `catalog.json` already follows. Its two lists stop
      being hand-maintained.
- [ ] `crates/catalog/tests/embedded_operations.rs` keeps working: it exists to catch a forgotten
      line, and once the line cannot be forgotten it should assert the *generated* index matches
      `providers/` instead. It must not become vacuous.
- [ ] **A scoped build is what an implementor runs.** `build --provider <id>` produces every
      per-provider artifact and touches no whole-catalogue one, so two implementors' write sets are
      disjoint. The story states the gate a provider implementor runs, and it does not include a full
      build.
- [ ] **`AGENTS.md` names the whole-catalogue artifacts as coordinator-owned**, alongside the
      existing generated-path table — the same status the board and `CHANGELOG.md` already have.
      A conflict in a generated file is resolved by regenerating, never by merging hunks.
- [ ] The whole-tree fixed-point property still holds: a full build after integrating N providers is
      a no-op, asserted by the existing test.
- [ ] Failing-first test: a scoped `build --provider <id>` leaves `generated.rs` and `catalog.json`
      byte-identical. That is the property the whole story rests on and nothing asserts it today.

## Progress
- Not started. Filed 2026-07-30 from a request to ship more connectors at once.

## Notes
- **This is the enabler; file it before the fleet.** With it, wave size for provider stories becomes
  disk-bound rather than structure-bound. Without it, five implementors produce five branches that
  conflict pairwise on one file and integrate serially anyway.
- The test lists were already derived by [C-54](C-54-derive-shipped-provider-lists.md) — it derived
  `tests/shipped_providers.rs` and `tests/shipped_providers_build.rs` from `providers/`. This is the
  same argument applied to the one list C-54 left behind, and C-54's reasoning is the precedent.
- **Disk is the next cap after this one.** Each worktree pays a cold Rust build; the integration
  tree's own `target/` is 5.6G and the filesystem is at 94%. `cargo clean` between waves, and treat
  five as the ceiling rather than the default.
