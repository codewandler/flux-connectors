---
id: C-424
title: "Fold the mechanical per-connector assertions into fleet-wide tests that cannot drift"
pillar: Build
status: backlog
priority: 3
design: docs/designs/generated-connector-tests.md
epic: generated-connector-tests
areas: [connector-flux]
note: "the likely answer to 'generate the boilerplate' is DELETE it — a test whose expected value comes from the same IR that produced the artifact asserts that the generator is the generator, and `diff` already checks all 557 artifacts byte-for-byte"
---

# Fold the mechanical per-connector assertions into fleet-wide tests that cannot drift

## Goal
Remove the duplication [C-423](C-423-classify-what-the-per-connector-tests-assert.md) measures, by
moving each mechanical claim into a test that reads `providers/*.toml` from disk — so it holds for
every connector including the ones not written yet, instead of for the 52 someone remembered.

## Acceptance
- [ ] Every bucket-(a) assertion C-423 found is either **moved** into a fleet-wide test or **deleted
      as already covered**, and the story records which, with counts. Nothing is dropped silently.
- [ ] A fleet-wide test enumerates `providers/*.toml` from disk, following the precedent
      `shipped_modules.rs` and `shipped_providers.rs` already set (C-54): the set is read from the
      directory, never listed in the test, so a **new** provider is covered the day it lands.
- [ ] Line count before and after is reported for `crates/connector-flux/tests/`, currently
      **22,455 across 52 files**.
- [ ] **No assertion is replaced by a tautological one.** A test whose expected value is derived from
      the same IR that produced the artifact asserts nothing —
      `cargo run -p connector-cli -- diff` already checks all 557 artifacts byte-for-byte against that
      derivation. Every surviving assertion names the independent thing it is checked against.
- [ ] Bucket-(c) reasoned claims are untouched. `slack_connector.rs`'s header explicitly warns a later
      reader not to tidy it; this story is that later reader and must not.

## Progress
- (not started)

## Notes
- **Blocked on C-423.** Without the classification this is a refactor by feel over 22,455 lines.
- **A confirmed instance of the problem, found by C-421 while it was doing something else:**
  `crates/connector-flux/tests/algolia_connector.rs` never loads the shipped `providers/algolia.toml`
  at all — all five of its load sites are inline fixtures. So whatever it asserts about Algolia is
  asserted about a fixture, not about what ships. That is the failure mode a per-connector file has
  and a disk-enumerating fleet-wide test structurally cannot.
- C-421 landed the shared seam these tests should use —
  `crates/connector-spec/tests/support/shipped_provider.rs` reads a provider's definition *and* its
  spec cache. Anything moved here should go through it rather than rolling a 19th loader.
