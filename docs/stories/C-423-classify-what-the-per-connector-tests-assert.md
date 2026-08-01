---
id: C-423
title: "Classify what the 52 per-connector test files actually assert"
pillar: Build
status: ready
priority: 2
design: docs/designs/generated-connector-tests.md
epic: generated-connector-tests
areas: [connector-flux]
note: "the spike that decides the epic, and it may end it — 22,455 lines across 52 files, but 37 of them declare a `const PROVIDER` and 31 a `const OPERATIONS`, which is the provider TOML said twice. The answer is a count, not an opinion"
---

# Classify what the 52 per-connector test files actually assert

## Goal
Produce the number that decides whether generating connector tests is worth doing at all: how much of
`crates/connector-flux/tests/*_connector.rs` restates the provider file, how much is already asserted
fleet-wide, and how much is a reasoned claim that must stay hand-written.

## Acceptance
- [ ] Every assertion in the 52 `*_connector.rs` files is classified into exactly one of three
      buckets, with a count and a percentage of lines for each:
      **(a) restates `providers/<name>.toml`** · **(b) already covered by a fleet-wide test** ·
      **(c) a specific reasoned claim**.
- [ ] Bucket (b) names the fleet-wide test that covers it. `shipped_modules.rs` enumerates
      `providers/*.toml` from disk and asserts every operation emits, parses, analyzes and is
      canonical; `shipped_providers.rs` is the loader-side equivalent. If a per-connector file asserts
      something one of those already asserts, that is the finding.
- [ ] Bucket (c) is quoted, not just counted — at least the five strongest examples, with what each
      would catch that nothing else would. `slack_connector.rs`'s "declares no query parameter at
      all" is one, and its own header says why: nothing else in the repo fails if someone converts a
      read to a GET.
- [ ] **The report may conclude "not worth generating", and that is a success.** If bucket (c)
      dominates, say so with the evidence and close the epic. A spike that returns a negative is the
      cheapest thing this backlog can buy.
- [ ] The findings land in this story's `## Progress` and in the design's `## Why`, so nobody
      re-opens the question on a hunch.

## Progress
- (not started)

## Notes
- **This is a measurement, not a refactor.** Change no test, delete no file, write no generator. The
  deliverable is a classification and a recommendation.
- Starting counts, already taken by the coordinator — verify rather than trust: 52 files, 22,455
  lines; `const PROVIDER` in 37, an env-var constant in 36, `const OPERATIONS` in 31,
  `const CREDENTIAL` in 29, `const BASE_URL` in 26.
- **The trap to name if you find it**: an assertion whose expected value is derived from the same IR
  that produced the artifact tests nothing, and `flux-connectors diff` already checks all 557
  artifacts byte-for-byte against that derivation. Flag every bucket-(a) case that is *already*
  tautological today.
