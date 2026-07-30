---
id: C-54
title: Derive the shipped-provider lists instead of hand-maintaining seven of them
pillar: Build
status: in-progress
priority: 4
design:
epic: connectors-v1
areas: [connector-spec, connector-flux, connector-cli, catalog]
note: caused a REWORK in the C-51/52/53 wave; five lists and two counts in four crates
---

# Derive the shipped-provider lists instead of hand-maintaining seven of them

## Goal
Make adding a provider cost one file, not seven places in five files across four crates — because the
duplication is not merely tedious, it silently drops coverage.

## Acceptance
- [x] The five `const SHIPPED` lists are gone, replaced by one derivation from `providers/`:
      `crates/connector-spec/tests/shipped_providers.rs:18` ·
      `crates/connector-flux/tests/shipped_modules.rs:24` ·
      `crates/connector-cli/tests/catalog_artifacts.rs:26` ·
      `crates/connector-cli/tests/shipped_providers_build.rs:27` ·
      `crates/connector-cli/tests/site_catalog.rs:32`.
      `crates/catalog/tests/embedded_operations.rs::the_provider_list_matches_the_repository` already
      derives from the directory — that is the pattern to generalise.
- [x] **Failing-first proof that the coverage hole is closed:** add a provider TOML without touching
      any test file, and every per-provider gate must pick it up. Today it does not — that is exactly
      how C-53 reached review with slack absent from one of the five lists, so
      `every_shipped_provider_compiles` and `every_shipped_operation_reaches_its_module` never ran for
      it.
- [x] The two hardcoded totals in `crates/catalog/tests/embedded_operations.rs` are derived, or become
      lower bounds. A per-provider curated count stays asserted explicitly — that is a deliberate
      inventory claim, unlike a total, which is just a sum.
- [x] Every assertion that exists today still exists and is no weaker. This story removes
      duplication, not coverage.
- [x] Provider-scoped behaviour survives: `build --provider <id>` must still be sound, which is why
      `crates/catalog/src/generated.rs` keeps a hand-written index (its own doc comment explains why).
      Say explicitly whether that index is in scope or deliberately left alone.

## Progress
- Filed 2026-07-30 at the close of the C-51/C-52/C-53 wave.
- **Done on `impl/C-54`.** All five `const SHIPPED` constants are replaced by a `shipped()` helper in
  their own file that reads `providers/*.toml`, sorts, and refuses an empty directory so a `for` loop
  cannot pass vacuously. `crates/catalog/tests/embedded_operations.rs` keeps its directory derivation
  and now shares it with `the_catalog_is_not_empty`, whose two totals (`6`, `38`) became comparisons
  against `providers/` and `crates/catalog/ops/`.
- The derivation is repeated per file rather than shared, deliberately: the four crates share no test
  crate, and creating one would mean a new workspace member plus a dev-dependency — both fenced for
  this story, and a runtime dependency `catalog` is contractually not allowed to take (AGENTS.md).
  What the story asked to remove is the duplicated *data*; the set of shipped providers now has one
  source of truth, the directory.
- New regression guard: `no_test_hand_maintains_a_shipped_provider_list`
  (`crates/connector-cli/tests/shipped_providers_build.rs`) fails if any `const`/`static` under
  `crates/*/tests` names two or more shipped providers. It is the failing-first test — at the merge
  base it named all five constants.
- **`crates/catalog/src/generated.rs` was deliberately left alone.** Its hand-written `mod` index is
  what keeps a provider-scoped `build --provider <id>` sound, and
  `the_provider_list_matches_the_repository` is the test that keeps it honest — the very pattern this
  story generalised.
- `operation_selection_stays_curated`'s per-provider counts are untouched and documented as
  deliberate. A provider added without a curated count is still covered by every other gate.

## Notes
- **Evidence, not a hunch.** Three implementors and two reviewers flagged this independently in one
  wave; it produced one REWORK and three separate merge conflicts on the same seven places. One
  implementor miscounted the lists as four in the very report complaining about the duplication —
  which is how the omission survived its own author's review.
- The conflict cost compounds with fan-out: N concurrent provider stories collide on all seven places,
  and the totals cannot be resolved by taking either side because each branch computed its number
  against a different baseline.
