---
id: C-221
title: Ship the Supabase connector
pillar: Spec
status: in-progress
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: the host is `https://{project_ref}.supabase.co` and there are TWO keys — `anon` (public, RLS-enforced) and `service_role` (bypasses row-level security entirely). Shipping the wrong default is a data-exposure decision"
---

# Supabase — a project-scoped host and two keys with very different authority

## Goal

Ship a curated `supabase` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

Supabase gives every project two keys. The `anon` key is safe to expose and is constrained by
row-level security. The `service_role` key **bypasses row-level security completely** and is
equivalent to database owner access.

Both are "the API key". A connector that declares one credential named `api_key` and lets an
operator paste either has made a security decision on their behalf without telling them — and the
catalogue's `risk` metadata would be describing the operation while saying nothing about the
authority the credential carries. This is the sharpest test yet of whether declared risk means
anything.

## Acceptance

- [ ] `providers/supabase.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [ ] A `verify` operation that is an argument-free read and runs unattended.
- [ ] `crates/connector-flux/tests/supabase_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/supabase.toml` exists.
- [ ] The scoped gate is green: `build --provider supabase`, `diff --provider supabase` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- **Declare the two keys as distinct credentials with distinct names**, and say in `help` exactly
  what `service_role` bypasses. Do not offer them as interchangeable alternatives of one mechanism.
- If the shipped operations only need `anon`, ship only `anon` and say why — the narrower connector
  is the better one, and it can be widened later with evidence.
- `{project_ref}` is a `[[config]]` field bound to the endpoint variable, the same shape as
  `zendesk`'s `{subdomain}`. Note that [C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md)
  applies: a host-position value is unvalidated at request time today.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.
