---
id: C-219
title: Ship the Confluence connector
pillar: Spec
status: ready
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: same host and same credential as the shipped `jira` connector (`{site}.atlassian.net`, Basic email+token). Two connectors, one authority — the credential-addressing model's first real collision"
---

# Confluence — a second Atlassian product sharing an authority with Jira

## Goal

Ship a curated `confluence` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

`jira` already ships with authority `com.atlassian.<site>` and a Basic email+token credential.
Confluence uses **the same host, the same account and the same token**, at a different path
(`/wiki/api/v2`).

This is the first time two connectors would legitimately share one authority, and it is the probe
the credential-addressing model has never had: does an operator who has connected Jira have to paste
the same token again for Confluence, or does the address resolve to the value they already supplied?
[C-90](C-90-credential-addressing-epic.md)'s whole point is that an address is a *place*, not a
per-connector copy — this is where that either pays off or is revealed as untested.

## Acceptance

- [ ] `providers/confluence.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [ ] A `verify` operation that is an argument-free read and runs unattended.
- [ ] `crates/connector-flux/tests/confluence_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/confluence.toml` exists.
- [ ] The scoped gate is green: `build --provider confluence`, `diff --provider confluence` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- **The answer to "does it reuse Jira's credential" is the story's real deliverable**, more than the
  operations are. Whatever it turns out to be, assert it in the contract test rather than describing
  it.
- Decide deliberately between a separate `confluence` connector and a second **service** on an
  `atlassian` connector. C-49 established services for exactly this shape (`google` has gmail,
  calendar, drive on one authority) — but `jira` already ships standalone, so a service split would
  mean moving it. Record the reason either way; do not let the existing layout decide by default.
- Confluence's v2 API paginates with an opaque cursor in a `Link` header. If that cannot be
  expressed, say so rather than shipping an operation that silently returns one page.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.
