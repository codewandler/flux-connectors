---
id: C-230
title: "Two per-provider tests assert catalogue-wide literals, so the next connector with the wrong shape turns an unrelated provider's test red from a worktree that cannot see it"
pillar: Build
status: in-progress
priority: 2
design:
epic:
areas: [build, connector-flux]
note: "one instance caught in review before merge on 2026-07-31 (C-216's prefix census, falsified by C-218 in the same wave); the C-216 implementor then found the identical defect already on main in trello_connector.rs:186-214, green only because no provider since Trello has placed a credential in the query string"
---

# Per-provider tests that assert catalogue-wide literals

## Goal

Make it impossible for one provider's contract test to be falsified by an unrelated provider
landing — the property that lets provider stories run in parallel at all.

## What was measured

**Instance 1, caught in review, never merged.** C-216's `the_catalogue_prefix_census_is_exactly_these_four`
walked every `providers/*.toml` and asserted equality against a four-element literal.
`providers/klaviyo.toml:208` declares a fifth non-empty `Header` prefix, and C-218 was landing in the
same wave. Proven by execution in a tree holding both branches:

```
left:  [... "klaviyo:klaviyo.api_key:Authorization:Klaviyo-API-Key " ...]  (5 entries)
right: [ ... ]                                                             (4 entries)
test result: FAILED. 5 passed; 1 failed
```

**Instance 2, already on `main`.** `crates/connector-flux/tests/trello_connector.rs:186-214` walks
every provider collecting each `AuthScheme::Query` credential and asserts:

```rust
assert_eq!(query_placed, [format!("{PROVIDER}:{KEY}"), format!("{PROVIDER}:{TOKEN}")], …);
```

It is green **today** only because no provider since Trello has placed a credential in the query
string. The next one that does turns *Trello's* test red. `resend_connector.rs` also defines a
`read_dir` helper over `providers/`.

## Why this class is expensive out of proportion to its size

`AGENTS.md` says provider stories run in parallel because "a provider story writes
`providers/<id>.toml` plus only per-provider artifacts, so two implementors' write sets are
disjoint". A catalogue-walking assertion breaks that guarantee **without touching a shared file**:

- it is invisible from the implementor's own worktree, which holds only their provider;
- it is invisible to the other implementor, whose diff is entirely disjoint;
- it is **not** among the eight whole-catalogue staleness failures `AGENTS.md` tabulates, so it does
  not read as expected;
- and the coordinator cannot resolve it the way the eight are resolved — those are *regenerated*,
  this is a hand-written literal in a shipped test.

It surfaces for the first time at integration, attributed to whichever merge happened to be second.

## The shape that works, already demonstrated

C-216's rework is the model and its reasoning generalises. The premise under test was a claim about
*specific connectors* — "Okta, PagerDuty and Statuspage already ship non-`Bearer` prefixes" — not a
claim about catalogue membership. So the test now loads those three **by name** and checks what each
declares. A fifth or fiftieth prefix cannot falsify it; one of those three changing its scheme word
*can*, which is exactly when the evidence would stop being true.

The second half of that assertion was **deleted rather than relocated**, because
`crates/connector-spec/tests/auth_prefix.rs::the_preset_schemes_carry_no_prefix_of_their_own`
already pins it and that file is entirely fixture-based — it never reads `providers/`, which is
precisely why it survives catalogue growth.

## Acceptance

- [x] **Failing-first test:** a guard that fails when a test under `crates/connector-flux/tests/`
      enumerates `providers/` and compares the result against a literal. Name it. It must fail on
      `trello_connector.rs:186-214` as that file stands today.
      → `connector-cli::per_provider_test_scope::no_per_provider_test_walks_the_provider_catalogue`.
      Failed at the merge base naming `trello_connector.rs` **and** `resend_connector.rs`.
      Deliberately wider than the acceptance in two ways, both recorded in its module docs: it
      governs `crates/*/tests/` rather than `connector-flux` alone, and it refuses the *walk* rather
      than the walk-plus-literal, because pairing a `read_dir` with a distant `assert_eq!` textually
      fails open.
- [x] `trello_connector.rs`'s query-placement census is reshaped so an unrelated provider placing a
      credential in the query string cannot falsify it. **Do not simply delete it** — C-159 §2's
      hazard account is real and the measurement has value; the question is where it belongs and what
      it should name.
      → Moved to `crates/connector-flux/tests/query_placed_credentials.rs` and turned from an
      equality into a property: *every* connector placing a credential in the query string declares
      no query parameter and emits no `?`. That is the combination C-159 §2's hazard is actually
      about. Trello keeps the closed, connector-scoped half
      (`trello_made_the_query_placement_hazard_reachable`).
- [x] `resend_connector.rs`'s `read_dir` helper is removed or justified.
      → Removed. The `without_config.len() > 1` count it fed became three named predecessors
      (`slack`, `github`, `openai`), the C-216 shape.
- [x] The rule is written where a provider implementor reads it — `AGENTS.md`, beside the
      parallel-provider design it protects — not only in a test comment.
      → `AGENTS.md` § "A per-provider test asserts about its provider, never about the catalogue",
      immediately after the eight-staleness-failures table and before the ninth (`COVERED_FLOOR`).
- [x] A catalogue-wide claim that genuinely deserves testing has a stated home, so this story does not
      read as "never measure the catalogue". `response_schema_coverage.rs` is the existing example of
      one done correctly: coordinator-owned, with a ratchet, and named in the fence.
      → The new `AGENTS.md` section carries a three-row table of homes: a universally quantified
      property in a whole-catalogue file, a claim about named connectors, and a coordinator-owned
      ratchet. The guard's own failure message names all three, so an implementor who trips it is
      told where the claim belongs.

## Notes

- **C-54's guard already covers the adjacent case and is worth reading first.**
  `connector-cli::shipped_providers_build::no_test_hand_maintains_a_shipped_provider_list` refuses a
  test `const` naming two or more shipped providers — it caught C-216's first rework draft, which had
  put the three predecessors in a `const PREDECESSORS`. The carve-out its own doc names is that a
  per-provider claim **inside a test body** is an assertion about each provider rather than a copy of
  the provider set. This story is the mirror image: C-54 stops a hand-maintained *list*, and nothing
  stops a hand-maintained *census*.
- The two instances differ in a way worth preserving: C-216's was caught by review before merge,
  Trello's has been latent on `main` since C-165. The second is the argument for a mechanical guard
  rather than reliance on review.

## Progress

**2026-07-31 — implemented on `impl/C-230`. Gate fully green: zero red tests.**

The guard, `crates/connector-cli/tests/per_provider_test_scope.rs`, classifies a test file as
per-provider when its stem begins with a shipped provider id at an `_` boundary (`trello_connector`,
`babelforce_ivr`) — **derived from `providers/`, so the guard keeps no inventory of its own** and
C-54's `no_test_hand_maintains_a_shipped_provider_list` has nothing to object to. It refuses
`read_dir` in such a file, scanning code lines only, so the module docs it asks authors to write may
name the marker in prose.

Two decisions worth re-reading before changing anything here:

1. **The line is the walk, not the literal.** Strictly, `resend`'s `without_config.len() > 1` was
   monotone and could not have been falsified by a provider landing. It was still removed. Detecting
   "compared against a literal" textually means pairing a `read_dir` with an `assert_eq!` several
   lines away and guessing whether the RHS is closed — a guard that is approximately right about the
   exact hazard fails open. And the deciding reason: the author of a per-provider test cannot review
   the population they quantify over, so a monotone claim written there is correct by luck, and the
   edit that turns it into an equality is one word.
2. **Trello's census was relocated and re-premised, not deleted.** The old question — "who else does
   this?" — has an answer that changes every time the catalogue grows, for reasons unrelated to the
   hazard. The new one — "does anyone combine a query-placed credential with caller text in the same
   query string?" — is what C-159 §2 is about, and it goes red exactly when the hazard account stops
   being true.

**Audited the whole test tree for further instances: there are none.** Every other catalogue walk
under `crates/*/tests/` is either universally quantified, a lower-bound floor, or derived-vs-derived
staleness. Two near-misses are deliberate and stay: `core_catalog.rs`'s `29/43/5` counts are the
vendored *flux core* catalogue, which no provider can change; and `shipped_providers.rs`'s
`operation_selection_stays_curated` iterates its own `[(provider, count)]` literal rather than a
walk, so a new provider simply makes no claim — the carve-out C-54's doc already names.

The guard's stated blind spots (a walk spelled with `glob`/`WalkDir`/a shell-out, or a catalogue
claim assembled from `include_str!`) are **currently vacuous** — no such construct exists anywhere in
the test tree — so nothing is hiding behind them today.

One file outside the story's named set was touched: `providers/trello.toml`'s header comment
cross-referenced the removed test by name. Comment-only, artifact-neutral (the full gate's
fixed-point assertions are green), and left dangling it would have been a false reference.
