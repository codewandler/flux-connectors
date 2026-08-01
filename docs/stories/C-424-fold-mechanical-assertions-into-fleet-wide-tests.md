---
id: C-424
title: "Fold the mechanical per-connector assertions into fleet-wide tests that cannot drift"
pillar: Build
status: backlog
priority: 3
design: docs/designs/generated-connector-tests.md
epic: generated-connector-tests
areas: [connector-flux]
note: "RE-CUT by C-423's measurement 2026-08-01. The target is real but SMALL — 209 assertions / 1,571 lines, under 7% of the corpus — and deleting it has a stated cost, because slack_connector.rs restates the fleet gate on purpose. A small careful pass, not the rewrite the epic imagined"
---

# Fold the mechanical per-connector assertions into fleet-wide tests that cannot drift

## Goal
Remove the one duplication [C-423](C-423-classify-what-the-per-connector-tests-assert.md) actually
found worth removing: **209 assertions across ~40 files** restating the fleet-wide emit-and-analyze
gate, concentrated in a near-identical `every_<x>_operation_emits_an_analyzable_module`.

**Read C-423's measurement before starting.** It re-scoped this story downward and the numbers matter:
bucket (b) is 1,571 lines, **7.0%** of the corpus — not the rewrite this story was filed imagining.

## Acceptance
- [ ] Every bucket-(a) assertion C-423 found is either **moved** into a fleet-wide test or **deleted
      as already covered**, and the story records which, with counts. Nothing is dropped silently.
- [ ] A fleet-wide test enumerates `providers/*.toml` from disk, following the precedent
      `shipped_modules.rs` and `shipped_providers.rs` already set (C-54): the set is read from the
      directory, never listed in the test, so a **new** provider is covered the day it lands.
- [ ] Line count before and after is reported for `crates/connector-flux/tests/`, measured at
      **22,595 across 52 files** (C-423; the 22,455 this story was filed with was hand-typed and 140 low).
- [ ] **`slack_connector.rs`'s restatement of the fleet gate is kept**, and the story says why it was
      kept while others went. Its own words: it restates the gate *"so that the Slack connector's own
      test file fails on its own"*. A deliberate duplication is not the same finding as an accidental one.
- [ ] **No assertion is replaced by a tautological one.** C-423 measured 194 of 2,352 sites (8.2%)
      as already unable to fail; the largest class is 73 restatements of
      `program.ops[0].name == operation.id`, where the emitted name *is* `operation.id` by
      construction. Removing those is in scope and is the cheapest real win here.
- [ ] **Do not repeat the coordinator's error that `diff` subsumes emitted-text assertions.** C-423
      disproved it: `diff` pins the emitted *text* against the committed rendering and says nothing
      about whether that text has a property — it would accept a committed rendering carrying a `?`
      just as happily. The Slack-family "no query string reaches the URL" claims are **not** covered.
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
