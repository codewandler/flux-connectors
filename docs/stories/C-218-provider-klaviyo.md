---
id: C-218
title: Ship the Klaviyo connector
pillar: Spec
status: done
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

- [x] `providers/klaviyo.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [x] A `verify` operation that is an argument-free read and runs unattended.
- [x] `crates/connector-flux/tests/klaviyo_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [x] **Failing-first test:** the contract test must fail before `providers/klaviyo.toml` exists.
- [x] The scoped gate is green: `build --provider klaviyo`, `diff --provider klaviyo` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
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

## Progress

**2026-07-31 — shipped on `impl/C-218`.** `providers/klaviyo.toml` (6 operations: 5 reads, 1 write),
`crates/connector-flux/tests/klaviyo_connector.rs` (11 tests), and the 9 per-provider artifacts
`build --provider klaviyo` emits. `diff --provider klaviyo` reports no drift.

**The decision this story asked for**, recorded in the provider header and pinned by
`the_revision_is_a_constant_and_not_an_operator_choice`: the revision is a `const_headers` entry,
**not** a `[[config]]` field with a default. The deciding argument is not "an operator has nothing to
choose" but that a `[[config]]` revision would make every `response_schema` in the file conditional
on a value the file does not control — the author's schemas described against one revision, the
operator's request carrying another, both looking correct. A fourth reason settles it mechanically:
`Level` is derived from what a field `binds`, there is no `header.*` binding, so a configured
revision is not merely unwise here — it is unspellable.

**One correction to the story's own framing.** The frontmatter says "nothing shipped has one". That
is wrong, and the provider header says so rather than repeating it: `const_headers` landed with C-55,
and `providers/notion.toml` has pinned the dated `Notion-Version: 2022-06-28` ever since
(`providers/anthropic.toml` pins `anthropic-version: 2023-06-01`). Klaviyo needed **no change to
`connector-spec`**. What is new is the *coupling* — the revision governs the JSON:API envelope and
the attribute set of every resource, so the pin and the schemas are one claim stated twice, and
`the_pin_and_the_schemas_name_one_date` fails if either is bumped alone.

**Eight whole-catalogue staleness tests are red and were left red**, as `AGENTS.md` requires. No full
build was run; the coordinator resolves them at integration.
