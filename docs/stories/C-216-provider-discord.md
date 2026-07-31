---
id: C-216
title: Ship the Discord connector
pillar: Spec
status: in-progress
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: the credential travels as `Authorization: Bot <token>` — not `Bearer`. C-184 landed AuthScheme prefixes; this is the first connector to use one that is not `Bearer`"
---

# Discord — the `Bot ` credential prefix, and a vendor whose ids are snowflakes

## Goal

Ship a curated `discord` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

[C-184](C-184-authscheme-prefix.md) taught `AuthScheme` to spell a credential with a prefix. Every
shipped connector that uses it spells `Bearer `. Discord requires `Bot ` for a bot token, and
`Bearer ` means something different there (a user OAuth token, with different permissions). A
connector that sends the wrong prefix does not fail loudly — it gets a 401 that reads like a bad
token, which is the worst kind of wrong.

This is the probe that turns the prefix from a parameter with one value into a parameter.

## Acceptance

- [ ] `providers/discord.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [ ] A `verify` operation that is an argument-free read and runs unattended.
- [ ] `crates/connector-flux/tests/discord_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/discord.toml` exists.
- [ ] The scoped gate is green: `build --provider discord`, `diff --provider discord` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- Bot tokens and OAuth2 bearer tokens are **different credentials with different capabilities**.
  Pick one and say why in the provider header; do not declare both mechanisms as if they were
  interchangeable alternatives.
- Discord ids are snowflakes — 64-bit integers that **must** be carried as strings. A schema typing
  a channel id as a number loses precision above 2^53 in any JSON consumer. Declare them as strings
  and say why.
- Rate limits are per-route and returned in headers. There is no IR field for that; record it in the
  operation `description` or state in Progress that it is unmodelled.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.
