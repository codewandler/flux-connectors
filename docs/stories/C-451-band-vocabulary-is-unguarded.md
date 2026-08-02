---
id: C-451
title: "A second table of wiring tokens sits outside the guard built for that vocabulary"
pillar: Bridge
status: ready
priority: 4
epic: host-explorer
areas: [host]
note: "found by the C-237 review. wiring_vocabulary.rs parses only the `const WIRING = {` block, so BANDS' five predicates are checked by nothing — a typo in one silently stops matching"
---

# A second table of wiring tokens sits outside the guard built for that vocabulary

## Goal

Bring the host page's `BANDS` table under the same guard that already protects `WIRING`, so a typo in
a band predicate fails a test instead of silently narrowing a filter.

## The finding

From the independent review of C-237:

> A second table of `wiring` tokens now lives outside the guard built for that vocabulary. `BANDS` at
> `crates/connectors-api/src/index.html:210-214` keys on `not-wired`, `partly-wired`, `wired`,
> `no-credential-required`, `no-credential`, but `crates/connectors-api/tests/wiring_vocabulary.rs:84`
> parses only the `const WIRING = {` block, so a typo in a band predicate is caught by nothing.

The Node suite exercises only two of the five (`not-wired`, `wired`) — `ui/test/host-page.test.mjs:763-788`.

**Why it matters more than a typo usually would:** the failure is silent and it points the wrong way.
A band that stops matching does not error; it renders an empty filter, and an operator reads "nothing
needs setup" as a fact about their tenant rather than as a broken predicate. That is the same shape as
the unpublished-field rule C-237 itself implements — never show absence as a fact.

## Acceptance

- [ ] `wiring_vocabulary.rs` (or a sibling) also parses `BANDS` and asserts every token it keys on is
      one `catalog::CredentialRequirement::as_str` really emits.
- [ ] **Failing-first:** rename one band token and watch the new assertion go red.
- [ ] The remaining three bands gain Node coverage, or the story records why two are sufficient.
- [ ] The gate is green.

## Progress
- (not started)

## Notes
- Do not merge `BANDS` into `WIRING`; they are different things — one maps a token to display text,
  the other groups tokens into filter bands. The guard is what should be shared, not the table.
- Two smaller review findings were judged not worth a story and are recorded here instead: `expand()`
  removes the open panel before awaiting with no `catch` (`index.html:489-493`), so a failed operation
  fetch leaves the operator with no panel and no message — pre-existing, not a regression; and C-237
  carried the C-408 unpublished-field rendering, which is beyond its Acceptance but aligned with a
  repo-wide rule and covered by Node test 11.
