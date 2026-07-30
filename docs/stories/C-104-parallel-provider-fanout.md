---
id: C-104
title: Make whole-catalogue artifacts coordinator-owned, so provider stories can run in parallel
pillar: Build
status: in-progress
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
- [x] `crates/catalog/src/generated.rs` is **generated on a full build** and leaves untouched by a
      `--provider` or `--service` run — the rule `catalog.json` already follows. Its two lists stop
      being hand-maintained.
      → `connector_cli::catalog::render_index` (`crates/connector-cli/src/catalog.rs:112`), planned
      in the full-run-only block at `crates/connector-cli/src/pipeline.rs:113`, path at
      `crates/connector-cli/src/workspace.rs:170`.
- [x] `crates/catalog/tests/embedded_operations.rs` keeps working: it exists to catch a forgotten
      line, and once the line cannot be forgotten it should assert the *generated* index matches
      `providers/` instead. It must not become vacuous.
      → `the_provider_list_matches_the_repository` still reads `providers/` from disk and compares it
      against the compiled-in index, which is now a **staleness** check on a committed artifact; a
      non-emptiness guard was added so it cannot pass against an empty tree. Measured red on a
      simulated new provider (see Progress), so it is not vacuous.
- [x] **A scoped build is what an implementor runs.** `build --provider <id>` produces every
      per-provider artifact and touches no whole-catalogue one, so two implementors' write sets are
      disjoint. The story states the gate a provider implementor runs, and it does not include a full
      build.
      → stated below and in `AGENTS.md`; verified by building a simulated 18th provider.
- [x] **`AGENTS.md` names the whole-catalogue artifacts as coordinator-owned**, alongside the
      existing generated-path table — the same status the board and `CHANGELOG.md` already have.
      A conflict in a generated file is resolved by regenerating, never by merging hunks.
      → `AGENTS.md`, "Whole-catalogue artifacts are coordinator-owned", with the four artifacts
      marked in the generated-path table.
- [x] The whole-tree fixed-point property still holds: a full build after integrating N providers is
      a no-op, asserted by the existing test.
      → `the_committed_tree_is_a_fixed_point_of_a_build` unchanged and green; it now also covers the
      index, and it was observed *failing* on the simulated new provider, so it stays meaningful.
- [x] Failing-first test: a scoped `build --provider <id>` leaves `generated.rs` and `catalog.json`
      byte-identical. That is the property the whole story rests on and nothing asserts it today.
      → `a_scoped_build_leaves_the_whole_catalogue_artifacts_byte_identical`,
      `crates/connector-cli/tests/catalog_index.rs`.

## The gate a provider implementor runs

Scoped, and deliberately **without a full build** — a full build would write the whole-catalogue
artifacts, which is the collision this story removes:

```bash
cargo run -p connector-cli -- build --provider <id>
cargo run -p connector-cli -- diff  --provider <id>
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

A story adding a **new** provider leaves exactly three tests red, measured rather than predicted:
`the_provider_list_matches_the_repository`, `the_catalog_is_not_empty` and
`the_committed_tree_is_a_fixed_point_of_a_build`. All three are whole-catalogue staleness checks, all
are red *because* the implementor correctly did not write a whole-catalogue file, and all three are
resolved by the coordinator's full build at integration. A story that only changes an existing
provider trips the third alone.

## Progress
- **Done** (impl/C-104). `crates/catalog/src/generated.rs` is generated by a full build and left
  untouched by `--provider`/`--service`. Its bytes are unchanged from the hand-written file apart
  from the header, so the change is purely in *who writes it*.
- The whole-catalogue set was determined empirically, not from the story's table: a full plan and a
  `--provider zendesk` plan were enumerated and differenced. The set is four, not two —
  `crates/catalog/src/generated.rs`, `web/public/catalog.json`, `web/public/v1/**/*.json` and
  `assets/readme-snippet-{light,dark}.svg`. The latter three were already full-run-only; only the
  index needed moving, and the new test covers all four so the class cannot quietly lose a member.
- Verified the fan-out property end to end by adding a simulated 18th provider, running
  `build --provider acmetest`, and confirming it wrote 12 per-provider artifacts and touched no
  whole-catalogue file. The three resulting red tests are recorded above and in `AGENTS.md`.
- Added a loud refusal for a provider name that is not a Rust identifier. `providers/` admits `-` in
  a file stem, and `pub(crate) mod google-ads;` does not parse — while the index was hand-written a
  human hit that immediately, but generated it would ship a `crates/catalog` that does not compile.
- Not done here: `CHANGELOG.md`, the board and `docs/roadmap.md` are coordinator-owned and untouched.

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
