---
id: C-218
title: Ship the Klaviyo connector
pillar: Spec
status: ready
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: every request MUST carry `revision: YYYY-MM-DD`. Omit it and the vendor refuses. This is a required constant header that is also a version pin — nothing shipped has one"
---

# Klaviyo — a mandatory dated API-revision header on every request

## Goal

Ship a curated `klaviyo` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

Klaviyo requires a `revision` header on **every** request, dated (`2024-10-15`), and a request
without it is refused. It is a constant header, which C-55 supports — but it is also a **version
pin**, which means the value is a claim this repository makes about which API contract its schemas
describe.

That is the interesting part. Every other connector's response schemas are implicitly "whatever the
vendor returns today". Here the connector states the version its schemas were written against, in a
header the vendor enforces. It is the closest thing in the catalogue to a checked compatibility
claim, and it is worth having one.

## Acceptance

- [ ] `providers/klaviyo.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [ ] A `verify` operation that is an argument-free read and runs unattended.
- [ ] `crates/connector-flux/tests/klaviyo_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/klaviyo.toml` exists.
- [ ] The scoped gate is green: `build --provider klaviyo`, `diff --provider klaviyo` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- The `revision` value and the response schemas must be written against the **same** date, and the
  provider header must say which date and why. A revision bumped without re-reading the schemas is
  worse than no revision at all.
- Decide whether `revision` is a constant header (`const_headers`) or a `[[config]]` field with a
  default. Constant is probably right — an operator has no basis to choose a date — but record the
  reasoning, because it is the first time the question has come up.
- Klaviyo's private API keys are scoped per-key. Say in `help` which scopes the shipped operations
  need.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.
