---
id: C-423
title: "Classify what the 52 per-connector test files actually assert"
pillar: Build
status: done
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
- [x] Every assertion in the 52 `*_connector.rs` files is classified into exactly one of three
      buckets, with a count and a percentage of lines for each:
      **(a) restates `providers/<name>.toml`** · **(b) already covered by a fleet-wide test** ·
      **(c) a specific reasoned claim**.
      → 2,352 assertion sites extracted mechanically; 1,774 hard assertions classified
      **(a) 636 / 35.9% · (b) 209 / 11.8% · (c) 929 / 52.4%**. Lines over all 22,595:
      **(a) 4,348 / 19.2% · (b) 1,571 / 7.0% · (c) 11,290 / 50.0%**, plus 5,386 / 23.8% shared
      preamble. See the design's `## Why`.
- [x] Bucket (b) names the fleet-wide test that covers it. `shipped_modules.rs` enumerates
      `providers/*.toml` from disk and asserts every operation emits, parses, analyzes and is
      canonical; `shipped_providers.rs` is the loader-side equivalent. If a per-connector file asserts
      something one of those already asserts, that is the finding.
      → every bucket-(b) site carries its owner: 136 `every_shipped_operation_reloads_as_a_composite_op`,
      160 `every_shipped_provider_loads`, 78 `…is_a_fixed_point_of_the_flux_formatter`,
      77 `every_shipped_operation_emits`, plus `operation_ids_are_declarable_in_flux` and
      `credential_paths.rs`.
- [x] Bucket (c) is quoted, not just counted — at least the five strongest examples, with what each
      would catch that nothing else would. `slack_connector.rs`'s "declares no query parameter at
      all" is one, and its own header says why: nothing else in the repo fails if someone converts a
      read to a GET.
      → seven quoted verbatim in the design: slack, sentry (trailing slash), google (C-56 clears a
      Drive field), hubspot (flat body answered 2xx), zoom (flattened settings answered 201),
      discord (the inert test it caught in itself), airtable (path safety, not query safety).
- [x] **The report may conclude "not worth generating", and that is a success.** If bucket (c)
      dominates, say so with the evidence and close the epic. A spike that returns a negative is the
      cheapest thing this backlog can buy.
      → **it does, and this is that negative.** Recommend closing C-424 as scoped and not opening a
      generator.
- [x] The findings land in this story's `## Progress` and in the design's `## Why`, so nobody
      re-opens the question on a hunch.

## Progress

**Done — and the answer is a negative: do not generate.** Measured at `e9ece54`. The full write-up
is in [the design's `## Why`](../designs/generated-connector-tests.md); this is the summary.

### The three counts

| Bucket | Assertions (of 1,774) | Lines (of 22,595) |
|---|---|---|
| (a) restates `providers/<name>.toml` | 636 · **35.9%** | 4,348 · **19.2%** |
| (b) already covered fleet-wide | 209 · **11.8%** | 1,571 · **7.0%** |
| **(c) a specific reasoned claim** | 929 · **52.4%** | 11,290 · **50.0%** |
| shared preamble (module doc, imports, consts, load helper) | — | 5,386 · 23.8% |

Counting only the 17,209 lines inside test functions: (a) 25.3% · (b) 9.1% · **(c) 65.6%**.

Three independently-built classifiers were hand-audited against 60 randomly sampled assertions
labelled by reading. The reported one agrees on **83%** with *balanced* errors (5 that should be (a),
4 that should be (c)); two rejected variants scored 85% and 82% but with errors 8–10 deep in one
direction. Across all three, (c) brackets at **50–63%** and (a) at **26–39%** — the conclusion does
not turn on the choice.

### Verified counts

52 files ✓. `const PROVIDER` 37 ✓ · env-var const 36 ✓ · `const OPERATIONS` 31 ✓ ·
`const CREDENTIAL` 29 ✓ · `const BASE_URL` 26 ✓ — **all five exactly as the coordinator had them.**
Line count is **22,595**, not 22,455; the brief's figure is 140 low at this base.

**The number that settles it:** all 277 `const` declarations in all 52 files occupy **656 lines —
2.9%** of the corpus. The duplication the epic was opened over is three per cent of the thing it was
proposed as a reason to rewrite.

### Why (c) dominates in kind, not only in count

- **369 of 384 distinct test-function names appear in exactly one file.** Only 15 names recur; the
  most-repeated reaches 10 of 52. There is no template to extract.
- **22.7% of the corpus is prose** (1,598 `//!` + 3,154 `///` + 368 inline). That prose is the
  argument each assertion rests on.
- **All 52 files cite a story id**; 90 distinct ids, median 5 per file. 29 cite C-30; 37 carry a
  `no_…` test asserting a deliberate *absence*. A generator emits what a provider declares — it
  cannot emit what a provider was deliberately not allowed to declare.
- C-230's `per_provider_test_scope.rs` already *enforces* that these files assert about their own
  provider and not the catalogue. They are governed as per-connector claims by rule.

### The two suspicions

**Tautology — 194 of 2,352 sites (8.2%), smaller than suspected.** Largest classes: 73 restatements
of `program.ops[0].name == operation.id` (the emitted name *is* `operation.id` by construction), 27
`!label.is_empty()`/`!help.is_empty()` and 22 secret-has-no-example (the loader refuses all three),
25 `user_env.is_empty() && user_suffix.is_none()` on a non-basic scheme (`validate_credentials`
refuses that too).

**A correction the brief needs:** `connector-cli -- diff` does **not** subsume assertions about
emitted text. `diff` pins the text against the committed rendering; it says nothing about whether
that text has a property, and would accept a committed rendering carrying a `?` just as happily. The
Slack-family "no query string reaches the URL" claims are not covered by it.

**Fixture drift — zero of 52.** C-421 closed this class before this measurement ran. `algolia` and
`linear` use fixtures because **no `providers/algolia.toml` or `providers/linear.toml` exists** —
they are recorded negative-result probes that assert the absence outright, and are bucket (c) end to
end (69% and 81% by assertion). `sendgrid` reads the real shipped file off disk but through
`provider::load` instead of the C-421 helper — a one-line follow-up, not drift in what it asserts.

### Recommendation

1. **Do not generate per-connector tests.** Close the generation idea; the premise that 22,595 lines
   are boilerplate is false.
2. **C-424 has a real but small target** — the 209 assertions / 1,571 lines of bucket (b),
   concentrated in a near-identical `every_<x>_operation_emits_an_analyzable_module` carried by ~40
   files. Under 7% of the corpus, and deleting it has a stated cost: `slack_connector.rs` restates
   the fleet gate *on purpose*, "so that the Slack connector's own test file fails on its own". Worth
   at most a small careful pass, not the rewrite the epic imagined.
3. **C-425 is untouched by this measurement and survives on its own merits.** The vendor
   `example`-versus-`response_schema` oracle is derived from two independent things, so the tautology
   finding does not reach it. Judge it separately.
4. One file is the outlier worth naming: `fly_connector.rs` (230 lines) carries a one-line module
   doc, no test doc comments, and no failure message beyond `"{} has a body"`. It is the only file in
   the 52 whose reasoning is absent rather than terse. If anything here should be reviewed by hand,
   it is that one — not generated, written.

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
