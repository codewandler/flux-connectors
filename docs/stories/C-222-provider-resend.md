---
id: C-222
title: Ship the Resend connector
pillar: Spec
status: in-progress
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: deliberately the simplest vendor in the fleet — Bearer token, fixed host, four obvious operations. Ships to establish the FLOOR, and to measure how much of a connector is boilerplate"
---

# Resend — the floor: how small can a good connector be

## Goal

Ship a curated `resend` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

Every other story in this wave was chosen because it forces something. This one is chosen because
it forces **nothing**, and that is the measurement: a plain Bearer token, one fixed host, no config
surface, no services, no pagination puzzle.

If a connector this simple still takes a large hand-authored TOML, that is evidence about the
authoring cost that [C-14](C-14-fetch-and-drift-check.md)'s spec ingest is meant to remove, and it
should be recorded as a number rather than a feeling. If it is genuinely small, that is the floor
every other provider story can be compared against.

## Acceptance

- [ ] `providers/resend.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [ ] A `verify` operation that is an argument-free read and runs unattended.
- [ ] `crates/connector-flux/tests/resend_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/resend.toml` exists.
- [ ] The scoped gate is green: `build --provider resend`, `diff --provider resend` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- **Record the size.** Lines of TOML, number of operations, and how much of it was mechanical. That
  measurement is half the reason this story exists, and it belongs in Progress.
- Do not pad the operation set to make the connector look substantial. Four good operations is the
  right answer if four is what is worth exposing.
- No `[[config]]` surface is needed. Confirm that a connector with none actually works end to end —
  every shipped example has at least one field, so the empty case may be untested.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.
