---
id: C-229
title: "A configuration field cannot declare one value reaching two positions, and it is the only thing still blocking Algolia"
pillar: Spec
status: ready
priority: 2
design: docs/designs/connector-configuration.md
epic:
areas: [connector-spec]
note: "measured by the C-164 implementor 2026-07-31, which refused to ship rather than declare the same question twice. Verified by the coordinator at provider.rs:806-811 — the declaration Algolia needs is refused by name: 'Two questions that share an answer are one question'"
---

# A configuration field cannot declare one value reaching two positions

## Goal

Let one collected value be substituted into more than one request position, so a vendor that puts its
tenant scope in two places can be expressed without asking the operator the same question twice.

## What was measured

[C-164](C-164-provider-algolia.md) was blocked on two clauses. **[C-187](C-187-config-cannot-pin-a-request-component.md) removed the first** — `Position::Header` exists at
`crates/connector-spec/src/config.rs:237`, `parse_binding` accepts `header.<name>`, and
`Binding::is_secret` returns `false` for `Self::Request { .. }`, so Algolia's application id no
longer needs a false `secret = true`. `Position`'s own documentation names this story's header as one
of its three motivating vendors.

**The second clause is untouched and is now the entire block.** Algolia's app id must reach the
`{app_id}-dsn.algolia.net` hostname *and* the `X-Algolia-Application-Id` header on every call. All
three ways to make one value reach both were measured:

| shape | outcome |
|---|---|
| two fields, different names (`endpoint.app_id` + `header.X-Algolia-Application-Id`) | **loads** — and is the problem: two host-side slots, one answer |
| two fields, one name | **refused** — shared-slot pass, `crates/connector-spec/src/provider.rs:795-820` |
| one field, header pin alone, hostname resolving from it | **refused** — only `Binding::Endpoint` binds a `base_url` variable, `provider.rs:831-855` |

The middle row is the finding, and the refusal is by name:

> configuration fields `X` and `Y` both resolve `{app_id}` … so a host would key them to one value
> under one slot. **Two questions that share an answer are one question** — bind one of them to a
> different name

## The rule is right; it is the expression that is missing

That refusal exists for a good reason (C-197): two fields keyed to one slot silently discard one
answer. It is not a bug to remove. But it is exactly what makes this connector unshippable, and the
rule and the vendor want opposite things.

The top row was weighed rather than dismissed. It fails on the configuration contract's own terms:
the second field has no honest `label` or `help` — the only truthful help text is *"type the same
value again"* — against the standing requirement that a connector asks for everything it needs and
nothing it cannot use.

**Why Algolia specifically:** of C-187's three motivating vendors it is the only one whose tenant
scope sits in *two* positions. Cloudflare's `zone_id` and Vercel's `teamId` sit in one each, which is
why those ship and this does not.

## Acceptance

- [ ] **Failing-first test:** one configuration field declares two destinations and one collected
      value reaches both. It cannot be declared today. Name it.
- [ ] The shape keeps **one field, one `name`, one host-side slot, one question** — that is what the
      shared-slot rule protects, and a fix that reintroduces two slots has solved nothing. A `binds`
      list, or an `also_binds`, are the candidates; record why the chosen one wins.
- [ ] **Settle which placeholder the emitted module carries when the two destinations spell the value
      differently.** `Position`'s `name` is deliberately both the placeholder *and* the wire
      spelling, so a multi-destination field forces this question. It is the design interaction most
      likely to be discovered late.
- [ ] The shared-slot refusal at `provider.rs:795-820` still fires for genuinely distinct fields that
      collide. Widening it into a hole is the failure mode; C-164's two boundary tests
      (`one_name_for_both_destinations_is_refused_as_a_shared_slot`,
      `a_header_pin_does_not_bind_the_hostname_template`) are the tripwires and must be updated
      deliberately, not deleted.
- [ ] `providers/algolia.toml` ships, C-164 closes, and its `status` moves off `blocked`. If it still
      cannot ship after this lands, that is a third measurement and belongs in C-164's Progress.
- [ ] Interaction with [C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md) is stated: one
      value reaching a hostname *and* a header must satisfy **both** position predicates, and the
      host predicate is the strict one. A value legal in a header and illegal in a hostname must be
      refused, not encoded differently per destination.

## Notes

- This is the third distinct gap the 2026-07-31 wave found in the configuration surface, and they are
  genuinely different: [C-225](C-225-a-config-field-cannot-declare-a-closed-set-of-values.md) is
  about the set of legal *values*, this is about the set of *destinations*, and C-214 is about
  *validating* the value where it is substituted. Read all three before designing any of them — one
  change could serve two, and three separate spellings would be the defect they each describe.
- C-164 is a **second documented refusal** and that is a successful outcome, not a failure. It now
  refuses with the space closed rather than surveyed, which is what makes this story writable.
