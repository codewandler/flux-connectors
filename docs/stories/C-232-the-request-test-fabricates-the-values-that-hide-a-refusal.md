---
id: C-232
title: "The whole-catalogue request test fabricates a value for every variable it discovers, so an operation that refuses against a real configuration passes it"
pillar: Build
status: ready
priority: 1
design:
epic:
areas: [build, connector-pack]
note: "found by C-110's review 2026-07-31: eight operations refused at build_request against an empty configuration while `cargo test --workspace` was fully green. The helper manufactures values for whatever it finds, so it can never discover that nothing supplies them"
---

# The request test fabricates the values that hide a refusal

## Goal

Make the whole-catalogue request check exercise the configuration an operator actually has, so an
operation that cannot compose a request fails a test instead of failing a customer.

## What was measured

C-110 shipped eight GraphQL operations whose pinned query documents contain braces.
`connector-pack` derives configuration variables by scanning every string literal for `{…}`
(`crates/connector-pack/src/request.rs:245-253`), so it read each GraphQL selection set as a list of
configuration placeholders. Against an **empty** configuration — the production shape, since the
provider declares no `endpoint.*` field — all eight refused:

```
linear-viewer: REFUSED `linear-viewer` needs `endpoint.id
    name
    displayName
    email
    admin` … and the bound configuration supplies none, so no URL composes
```

**And `cargo test --workspace --no-fail-fast` was fully green.**

The reason is the helper. `every_shipped_operation_builds_an_absolute_request`
(`crates/connector-pack/tests/request.rs:316-343`) asserts only on the resulting **URL**, and its
`configuration()` helper manufactures a value for every variable the scan *discovers*. So the test's
input is derived from the same scan whose output it is meant to check: whatever the pack decides it
needs, the test supplies. It cannot fail for a missing value, because a value is never missing.

## Why this is priority 1

It is a test whose shape makes a whole class of defect **undetectable**, and the class is
"the connector cannot make a single call". That is the most consequential failure a connector can
have and the cheapest to notice, if anything looked.

It also held while a documented invariant was falsified. `request.rs:57-64` states that the
brace-carrying string literals in the shipped catalogue "are of exactly two kinds, and both are
configuration" — the templated base URLs and C-187's pin binds. A third kind arrived and nothing
reported it.

## Acceptance

- [ ] **Failing-first test:** every shipped operation composes a request against the configuration an
      operator would actually supply — that is, the fields the provider file **declares**, and
      nothing else. It must fail for any operation that refuses. Name it.
- [ ] The configuration under test is built from `[[config]]` **declarations**, never from the
      variables the scan discovers. Deriving the input from the thing under test is the defect; a
      fix that keeps that dependency has not fixed it.
- [ ] An operation that declares no configuration is exercised **against an empty configuration**,
      because that is its production shape. This is the case that was never run.
- [ ] The assertion covers more than the URL. A request whose URL composes while its body has been
      rewritten by configuration substitution is the second half of what C-110's review found, and a
      URL-only check cannot see it.
- [ ] The existing `every_shipped_operation_builds_an_absolute_request` is repaired or replaced, not
      supplemented. Two whole-catalogue request tests where one lies is worse than one that is
      honest.
- [ ] The invariant comment at `crates/connector-pack/src/request.rs:57-64` is either re-established
      as true or rewritten to describe what actually holds — and something checks it, rather than it
      being prose that a future connector can silently falsify.

## Notes

- **This is a different defect from the one that caused it.** Whether a GraphQL document's braces
  should be opaque to configuration scanning is C-110's question. This story is that *nothing
  noticed*, which would be worth fixing even if C-110 is withdrawn entirely — the next connector with
  a brace in a literal hits the same blind spot.
- Related in kind to [C-228](C-228-two-auth-tests-no-longer-reach-the-checks-they-name.md) and
  [C-230](C-230-per-provider-tests-hold-catalogue-wide-literals.md): the 2026-07-31 wave produced
  five separate instances of a test that passes without exercising its subject. This one is the most
  expensive because its subject is "does the connector work at all".
- Worth checking while in here: `crates/connector-pack/tests/request.rs` is the natural home, but the
  differential test C-145 added compares the pack's request against the shipped Flux for all
  operations. If that one shares the fabricating helper, it has the same hole.
