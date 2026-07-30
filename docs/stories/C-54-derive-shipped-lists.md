---
id: C-54
title: Derive the shipped-provider lists instead of hand-maintaining seven of them
pillar: Build
status: ready
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
- [ ] The five `const SHIPPED` lists are gone, replaced by one derivation from `providers/`:
      `crates/connector-spec/tests/shipped_providers.rs:18` ·
      `crates/connector-flux/tests/shipped_modules.rs:24` ·
      `crates/connector-cli/tests/catalog_artifacts.rs:26` ·
      `crates/connector-cli/tests/shipped_providers_build.rs:27` ·
      `crates/connector-cli/tests/site_catalog.rs:32`.
      `crates/catalog/tests/embedded_operations.rs::the_provider_list_matches_the_repository` already
      derives from the directory — that is the pattern to generalise.
- [ ] **Failing-first proof that the coverage hole is closed:** add a provider TOML without touching
      any test file, and every per-provider gate must pick it up. Today it does not — that is exactly
      how C-53 reached review with slack absent from one of the five lists, so
      `every_shipped_provider_compiles` and `every_shipped_operation_reaches_its_module` never ran for
      it.
- [ ] The two hardcoded totals in `crates/catalog/tests/embedded_operations.rs` are derived, or become
      lower bounds. A per-provider curated count stays asserted explicitly — that is a deliberate
      inventory claim, unlike a total, which is just a sum.
- [ ] Every assertion that exists today still exists and is no weaker. This story removes
      duplication, not coverage.
- [ ] Provider-scoped behaviour survives: `build --provider <id>` must still be sound, which is why
      `crates/catalog/src/generated.rs` keeps a hand-written index (its own doc comment explains why).
      Say explicitly whether that index is in scope or deliberately left alone.

## Progress
- Not started. Filed 2026-07-30 at the close of the C-51/C-52/C-53 wave.

## Notes
- **Evidence, not a hunch.** Three implementors and two reviewers flagged this independently in one
  wave; it produced one REWORK and three separate merge conflicts on the same seven places. One
  implementor miscounted the lists as four in the very report complaining about the duplication —
  which is how the omission survived its own author's review.
- The conflict cost compounds with fan-out: N concurrent provider stories collide on all seven places,
  and the totals cannot be resolved by taking either side because each branch computed its number
  against a different baseline.
