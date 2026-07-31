---
id: C-215
title: Ship the Mailchimp connector
pillar: Spec
status: ready
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: the base URL is `https://{dc}.api.mailchimp.com/3.0`, where `{dc}` is a datacenter suffix baked into the API key itself (`...-us14`). Basic auth with a FIXED username (`anystring`) and the key as the password"
---

# Mailchimp — a datacenter-suffixed host and a fixed-username Basic credential

## Goal

Ship a curated `mailchimp` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

Mailchimp's host is `https://{dc}.api.mailchimp.com/3.0`, and `{dc}` is not something an operator
looks up — it is the suffix of their own API key (`abc123...-us14` means `us14`). That is a
configuration value **derived from a credential**, which nothing in this catalogue has met.

Its Basic credential is the second unmet shape: the username is the literal string `anystring` and
the password is the key. `Acquisition::BasicJoin` exists and `needs_username` is published, but no
shipped connector has a Basic mechanism whose username is a **constant rather than operator-supplied**.

## Acceptance

- [ ] `providers/mailchimp.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [ ] A `verify` operation that is an argument-free read and runs unattended.
- [ ] `crates/connector-flux/tests/mailchimp_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/mailchimp.toml` exists.
- [ ] The scoped gate is green: `build --provider mailchimp`, `diff --provider mailchimp` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- The `{dc}` value must be a `[[config]]` field bound to the endpoint variable. **Do not try to
  derive it from the credential** — that would mean reading a secret to compose a URL, and the
  redactor would then hold a value that is part of the host. Ask the operator for it and say in
  `help` where to find it (it is the part after the dash in their key).
- Decide and record whether `anystring` is spelled as a constant in the mechanism or asked of the
  operator. Asking for a value the vendor documents as arbitrary is a worse operator experience;
  hard-coding it is a claim about the vendor that must be sourced in a comment.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.
