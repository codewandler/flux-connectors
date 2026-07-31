---
id: C-232
title: "The whole-catalogue request test fabricates a value for every variable it discovers, so an operation that refuses against a real configuration passes it"
pillar: Build
status: done
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

- [x] **Failing-first test:** every shipped operation composes a request against the configuration an
      operator would actually supply — that is, the fields the provider file **declares**, and
      nothing else. It must fail for any operation that refuses. Name it.
      → `every_declared_operation_composes_a_request_from_its_declared_configuration`
      (`crates/connector-pack/tests/request.rs`). The failing-first proof is the unit test that
      drives the same refusal over C-110's document,
      `a_braced_literal_that_is_neither_a_url_nor_a_pin_is_refused`
      (`crates/connector-pack/src/request.rs`), which is red at the base.
- [x] The configuration under test is built from `[[config]]` **declarations**, never from the
      variables the scan discovers. Deriving the input from the thing under test is the defect; a
      fix that keeps that dependency has not fixed it.
      → `declared_config` / `declared_for` / `configuration` read `providers/<id>.toml`'s
      `[[config]]` blocks — `binds` for the variable, `example` for the value — and bind those and
      nothing else. The old `configuration()`/`value_for` pair is gone.
- [x] An operation that declares no configuration is exercised **against an empty configuration**,
      because that is its production shape. This is the case that was never run.
      → falls out of the above, and is asserted explicitly: the test counts service modules whose
      declared configuration is empty and fails if there are none.
- [x] The assertion covers more than the URL. A request whose URL composes while its body has been
      rewritten by configuration substitution is the second half of what C-110's review found, and a
      URL-only check cannot see it.
      → every operation is built twice against two different declared configurations and the body
      and headers must be identical. Measured against a restored fixture: the assertion fires with
      `left: Some("{"query":"a-document"}") / right: Some("{"query":"xa-document"}")`.
- [x] The existing `every_shipped_operation_builds_an_absolute_request` is repaired or replaced, not
      supplemented. Two whole-catalogue request tests where one lies is worse than one that is
      honest.
      → replaced. The name, the `configuration()` helper, `value_for` and `resolved_host` are all
      deleted.
- [x] The invariant comment at `crates/connector-pack/src/request.rs:57-64` is either re-established
      as true or rewritten to describe what actually holds — and something checks it, rather than it
      being prose that a future connector can silently falsify.
      → re-established as a rule. `unconfigurable()` classifies every brace-carrying literal into
      the two declared kinds and `refuse_unconfigurable()` returns `Error::Unbuildable` for anything
      else, at projection and again at build. `sole_placeholder` was tightened to a configuration
      field name so a JSON object literal is not read as a pin.

## Progress

**2026-07-31 — implemented on `impl/C-232`, together with C-233 as the story asks.**

The root fix is in `crates/connector-pack/src/request.rs`: a brace in a bound string literal is read
as configuration only for the two kinds the module always claimed — a templated URL (`://`) and a
C-187 pin bind (a sole placeholder whose inner text is a configuration field name). Anything else is
`Error::Unbuildable`, raised in `Operation::project` and again in `request::build`. That is what
removes the circularity at its source: the scan can no longer *invent* a variable out of a vendor's
syntax, so a test that binds what the scan reports is binding declarations.

The whole-catalogue test is driven from `connectors/*.connector.toml` and `crates/catalog/ops/`
rather than from `catalog::operations()`, which is what lets it cover a provider that is not in the
coordinator-owned index — the C-233 half. `the_declared_configuration_agrees_with_every_templated_base_url`
is the oracle for the small line-oriented `[[config]]` reader: it cross-checks the provider file
against the emitted manifest, from two different artifacts, so a mis-read is loud rather than
vacuous.

Left for a follow-up: the reader exists only because `[[config]]` reaches no artifact
(`AGENTS.md`, "Six declarable surfaces reach no artifact at all"). C-87 deletes it.

**2026-07-31, round 1 rework** — review passed (299 operations byte-identical across the merge base
and head, 14 of 15 mutations caught); four comments asserting things that were not true, which this
repository treats as a defect.

- **The pin-name grammar was reconciled toward the loader.** `crates/connector-spec/src/config.rs`
  requires a `path.`/`query.`/`header.` binding suffix to be non-empty and nothing else, so the
  identifier-only predicate here rejected pins the loader accepts — measured: `binds =
  "query.page.size"` emits `page_size = "{page.size}"` and was reported as a document. `is_pin_name`
  now admits every loader-legal name **except** one carrying whitespace or a `"`, because a JSON
  object literal necessarily quotes its keys and dropping that clause re-admits `{"already":
  "json"}` as a pin named after its own contents. The loader is authoritative; the one extra clause
  and its reason are stated at `is_pin_name`, and `the_pin_name_rule_is_narrower_than_the_loaders`
  pins the divergence.
- **The doubled refusal is no longer a claim.** `refuse_unconfigurable` is called in
  `Operation::project` and in `request::build`, and only the second was executed — deleting the
  first left the workspace green.
  `a_document_literal_is_refused_at_projection_and_not_only_at_build` closes it by doctoring a
  shipped entry's `flux`, the technique `differential.rs` already uses. Verified by mutation in both
  directions.
- **The manifest comment** claiming this crate "never sees `providers/*.toml`" now says what is
  true: no *code* reads one and there is no `connector-spec` edge, and the tests do read them.
- A doc comment was glued to the wrong item, and `example`'s missing oracle is now recorded at
  `declared_config` rather than left unsaid.

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
