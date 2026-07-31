---
id: C-220
title: Ship the New Relic connector
pillar: Spec
status: ready
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: US and EU accounts live on DIFFERENT hosts (`api.newrelic.com` vs `api.eu.newrelic.com`) and the key does not say which. A wrong choice fails as an auth error, not a routing error"
---

# New Relic — a region-selected host chosen by the operator, not the vendor

## Goal

Ship a curated `newrelic` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

Nine shipped connectors template a host, but every one of them templates a *label the operator
owns* (`{subdomain}.zendesk.com`). New Relic is different: the operator picks between two
**vendor-owned** hosts, and the credential does not disclose which one it belongs to. Choosing wrong
returns a 401 that looks exactly like a bad key.

That makes it the case for a config field whose value comes from a **closed set** rather than free
text. Nothing in the IR expresses "one of these two", and shipping this connector will either
produce that expression or record honestly that an operator can type anything into a field where
only two values work.

## Acceptance

- [ ] `providers/newrelic.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [ ] A `verify` operation that is an argument-free read and runs unattended.
- [ ] `crates/connector-flux/tests/newrelic_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/newrelic.toml` exists.
- [ ] The scoped gate is green: `build --provider newrelic`, `diff --provider newrelic` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- The enum question is the story's finding, not a side note. If `[[config]]` cannot express a closed
  set, **file it** rather than working around it with `help` text — a free-text field with two valid
  values is the same class of defect as the `no-credential` conflation C-206 just fixed.
- Prefer the REST v2 API over NerdGraph for the first pass. NerdGraph is GraphQL and belongs with
  [C-110](C-110-provider-linear.md)'s decision, not ahead of it.
- Do not model the query-language endpoints (NRQL) as operations. A free-form query string is not a
  curated operation and would make the connector's surface unbounded.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.
