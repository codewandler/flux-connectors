---
id: C-217
title: Ship the Bitbucket connector
pillar: Spec
status: ready
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: every operation is scoped to a `{workspace}` PATH SEGMENT. C-187 just landed `binds = "path.<name>"` and nothing ships one — this is its first real consumer"
---

# Bitbucket — the first consumer of C-187's pinned path segment

## Goal

Ship a curated `bitbucket` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

[C-187](C-187-config-cannot-pin-a-request-component.md) landed `binds = "path.<name>"` so an
operator can pin a tenant scope at install time. **No shipped connector uses it.** A capability with
no consumer is a capability nobody has checked.

Bitbucket is the honest first consumer: every meaningful endpoint is under
`/2.0/repositories/{workspace}/...`, and a workspace is exactly the "once per installation, not once
per call" value C-187 exists for. Without the pin, every operation would carry a `workspace`
argument a model chooses each time — the Cloudflare problem, again.

## Acceptance

- [ ] `providers/bitbucket.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [ ] A `verify` operation that is an argument-free read and runs unattended.
- [ ] `crates/connector-flux/tests/bitbucket_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/bitbucket.toml` exists.
- [ ] The scoped gate is green: `build --provider bitbucket`, `diff --provider bitbucket` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- Pin `workspace` as a path segment. That makes one installed connector address one workspace,
  which is the intended consequence — say so in the provider header the way `cloudflare.toml` does.
- **Read [C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md) before writing the config
  help text.** A pinned path value is not validated at request time yet, so a workspace containing a
  slash produces a wrong URL rather than a refusal. Do not paper over it; if it bites your test,
  that is C-214's finding and worth reporting.
- Bitbucket Cloud and Bitbucket Server/Data Center are different APIs with different hosts. Ship
  Cloud and say that Server is out of scope, rather than leaving a reader to discover it.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.
