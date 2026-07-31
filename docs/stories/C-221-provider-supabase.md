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

- [x] `providers/supabase.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [x] A `verify` operation that is an argument-free read and runs unattended.
- [x] `crates/connector-flux/tests/supabase_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [x] **Failing-first test:** the contract test must fail before `providers/supabase.toml` exists.
- [x] The scoped gate is green: `build --provider supabase`, `diff --provider supabase` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
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

## Progress

**2026-07-31 — implemented on `impl/C-221`; ready for coordinator integration.**

The probe's answer: **one credential, `supabase.anon_key`, and `service_role` is declared nowhere
and explained anyway.** Every shipped operation is a `GET` the `anon` role satisfies, so the narrow
connector is the correct one and the bypass key has no slot to be pasted into. What the connector
can actually do about the choice is the credential's *name* and the `help` text next to the input
box — both are asserted in `crates/connector-flux/tests/supabase_connector.rs` rather than left as
prose, because prose is what decays.

**Shipped** — 3 curated reads, all `low`, all `idempotent`:

| operation | request | why |
|---|---|---|
| `supabase-schema-describe` | `GET /rest/v1/` | PostgREST's OpenAPI 2.0 description of the exposed schema; also `verify` |
| `supabase-rows-list` | `GET /rest/v1/{table}` | one path param, one bounded integer `limit`, no response schema (the shape is the operator's own database) |
| `supabase-auth-settings` | `GET /auth/v1/settings` | which sign-in methods the project has enabled; configuration only |

Address is `com.supabase.api:v1`; `{project_ref}` is bound by a `[[config]]` field, one implicit
`default` service (REST and Auth share host, version and key).

**What the preserved WIP test was changed to.** `a7dde26` carried the 502-line test and no provider
TOML. It was written at a base 40 commits stale and was unreviewed. Audited against post-merge
`main`: it compiles unchanged and all seven of its assertions are sound, so its substance is
untouched. One test was **added** — `no_caller_supplied_value_reaches_the_request_as_free_text` —
because the file asserted nothing about the query string while `providers/supabase.toml`'s curation
argument rests entirely on the claim that no free-text query value ships. PostgREST's whole
expressive power is its query string, so that was the largest unpinned claim in the story.

**Deliberately not shipped, and named in the provider header:** PostgREST's `select`, `order` and
column filters (query-encoding gap — a column filter is additionally a caller-supplied parameter
*name*, which the IR cannot declare at all); `offset` (incoherent without `order`); all writes to
`/rest/v1/{table}` and `POST /rest/v1/rpc/{fn}`; the `/auth/v1` identity flow and admin endpoints;
`/storage/v1`, Realtime and Edge Functions; `[[events]]`/`[[channels]]`.

**The eight whole-catalogue staleness tests are red as designed** and were reported, not silenced.
The ninth, `the_recorded_floor_is_the_measured_figure`, is **green** for this story alone: coverage
goes 225/254 → 227/257 against `COVERED_FLOOR = 220` and slack 25. It may still cross during the
wave's *accumulation* — that remains the coordinator's call.

**One inference a later reader should challenge first.** This connector declares the key in the
`apikey` header only, and not additionally in `Authorization: Bearer`, which the vendor's own `curl`
examples send. The reasoning is in the provider header; if it is wrong the connector fails closed
with a `401` rather than succeeding with reduced authority. It is a statement about gateway
behaviour rather than a documented request contract, and it is the most likely thing here to drift.
