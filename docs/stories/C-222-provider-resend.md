---
id: C-222
title: Ship the Resend connector
pillar: Spec
status: done
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

- [x] `providers/resend.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent. → four operations; the exclusions (batch send, scheduled send with
      update/cancel, API keys, audiences/contacts/broadcasts, webhooks) are each named with a reason
      in the file header.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads. → asserted by
      `every_operation_declares_its_risk_its_idempotency_and_its_response_shape`; effects are derived
      by the emitter (`effects ["network"]` on all four).
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before. → **satisfied vacuously and
      deliberately: there is no `[[config]]` surface**, which the Notes sanction. The secret half is
      still enforced positively by `no_token_shaped_value_appears_in_the_source_or_in_the_emitted_flux`.
- [x] A `verify` operation that is an argument-free read and runs unattended. →
      `resend-domain-list`, pinned by `verify_is_an_argument_free_read`.
- [x] `crates/connector-flux/tests/resend_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses. → seven tests, five of which assert on emitted
      Flux rather than on the declaration alone.
- [x] **Failing-first test:** the contract test must fail before `providers/resend.toml` exists. →
      all 7 fail at `0f23b56` with `cannot read …/providers/resend.toml (No such file or directory)`.
- [x] The scoped gate is green: `build --provider resend`, `diff --provider resend` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
      → `7 artifacts up to date (1 provider checked)`; workspace build, clippy and fmt clean.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration. → exactly the eight tabulated, across the five tabulated binaries. The
      coordinator-owned ninth (`the_recorded_floor_is_the_measured_figure`) is **green**.

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

## Progress

**Done.** Four operations, gate green, exactly the eight tabulated staleness tests red.

### The measurement — how small a good connector is

`providers/resend.toml` is **282 lines, of which 90 are declaration** (166 comment, 26 blank), for
**4 operations** — 1 write, 3 reads. That is the smallest connector in the shipped fleet by
declaration lines:

| provider | operations | declaration lines | lines/op |
|---|---|---|---|
| **resend** | **4** | **90** | **22.5** |
| sendgrid | 4 | 102 | 25.5 |
| trello | 6 | 148 | 24.7 |
| github | 5 | 157 | 31.4 |
| postmark | 6 | 207 | 34.5 |

**So the floor is roughly 22 declaration lines per operation, and it barely moves across the fleet.**
The per-operation cost is near-constant at 22–34 lines regardless of how much the vendor forces;
what varies between connectors is almost entirely the *operation count*, not the cost of each.

**How much was mechanical.** Of the 90 declaration lines, 25 are the operation frame
(`[[operations]]`, `id`, `method`, `path`, `risk`, `idempotency`), 6 are parameter blocks, 4 are
`[operations.response_schema]` headers whose `properties` collapse into 4 further lines (3 of them
over 300 characters), 12 are `description`, and the remaining ~13 are provider scalars and the one
`[[auth]]` block. **Call it a third mechanical** — the frame and the parameter blocks are
transcription an endpoint-shaped ingest (C-14) could do outright, and the response schemas are
another chunk an OpenAPI document would carry if Resend published one.

**What ingest could not have carried, and this is the finding:** the 12 `description` strings written
as model-facing contracts, the `risk`/`idempotency` judgement, the curation decision itself (which of
~20 documented endpoints deserve to exist), and the `User-Agent` below. The comment-to-declaration
ratio is **166:90, nearly 2:1** — the reasoning about a connector is consistently larger than the
connector. C-14 removes the third that is transcription; it does not touch the two-thirds that is
judgement.

### The one thing the floor forced: a `User-Agent`

Resend rejects any request without a `User-Agent` with `403`, valid key and all. The connector
declares `const_headers = { "User-Agent" = "flux-connectors" }`, asserted onto every emitted request
by `every_emitted_request_carries_the_user_agent_resend_demands`.

**This was re-derived rather than inherited.** The interrupted agent's file justified the header by
citing `AGENTS.md`'s "Intentional gaps" entry — that no HTTP implementation is bound here, so whether
a host would supply one "cannot be checked". **That entry was already marked CLOSED on 2026-07-31.**
`codewandler-flux-web` is in `Cargo.lock`, and `connectors-api` binds `HttpRequestTool` as its
`Egress` (`crates/connectors-api/src/state.rs:108`). So it *is* checkable, and it was checked: both
`reqwest::Client` builders in `codewandler-flux-web` (`egress.rs:19-29` and `egress.rs:153-158`) omit
`ClientBuilder::user_agent`, `WebOptions` carries no field for one, and reqwest sends no default. **A
Resend call through the host this repository actually ships would go out bare and come back `403`.**
The header is declared because it was verified missing, not because it was unverifiable — a stronger
result than the one it replaces, and the justification in both the TOML header and the test was
rewritten to say so.

### Audit of the preserved WIP (`8ca38eb`)

The handoff warned the test and the TOML might disagree on the curated set. **They did not** — both
name the same four operations in the same order, and all 7 tests pass against the TOML unmodified.
Two things were changed: the stale `User-Agent` justification above, and one wrong assertion message
claiming "both writes here are `POST`" when the connector has exactly one write.

### Adjacent, not fixed

Two emitter gaps bound this connector and are the reason it is four operations rather than seven:
**there is no optional body field** (`body_tree` sends every declared field, so an omitted one
travels as explicit `null`), which keeps `cc`/`bcc`/`reply_to`/`text`/`scheduled_at` out and with
them the cancel and reschedule operations; and **query values are interpolated unencoded**, the
standing `zendesk-ticket-search` gap. The first is the one that actually costs operations here and
deserves its own story.
