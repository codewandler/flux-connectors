---
id: C-216
title: Ship the Discord connector
pillar: Spec
status: done
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

- [x] `providers/discord.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [x] A `verify` operation that is an argument-free read and runs unattended.
- [x] `crates/connector-flux/tests/discord_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [x] **Failing-first test:** the contract test must fail before `providers/discord.toml` exists.
- [x] The scoped gate is green: `build --provider discord`, `diff --provider discord` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
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

## Progress

**2026-07-31 — shipped on `impl/C-216`.** 6 operations (5 reads, 1 write), one credential, one
config field. Scoped gate green; the eight whole-catalogue staleness tests `AGENTS.md` tabulates are
red and left for the coordinator, and the ninth (`the_recorded_floor_is_the_measured_figure`) is
**green** — this story's response shapes fit inside the `COVERED_FLOOR` slack on their own.

**The story's premise was wrong, and the correction is asserted rather than filed away.** This story
says every shipped connector using the prefix axis spells `Bearer `. Measured over `providers/*.toml`
by `the_catalogue_prefix_census_is_exactly_these_four`, that was false in both directions: okta
(`SSWS `), pagerduty (`Token token=`) and statuspage (`OAuth `) already shipped non-`Bearer` prefixes,
and **no** connector spells `Bearer ` as a `Header` prefix at all, because `AuthScheme::Bearer` is a
preset variant. So Discord is not the first non-`Bearer ` prefix.

What it *is* — and why the probe still earns its place — is the first prefix whose **neighbouring
value is also valid vendor syntax for a different credential**. `SSWS <token>` sent as `Bearer` is
rejected by a vendor with no bearer scheme; `Bot <token>` sent as `Bearer` is a *well-formed* Discord
request for an OAuth2 user principal the caller does not hold, answered with a 401 indistinguishable
from a revoked token. The prefix is pinned character for character, trailing space included.

**Rate limits are unmodelled, deliberately.** Discord's are per-route with a bucket per major path
parameter, discovered from `X-RateLimit-*` and `Retry-After` headers. `Quirks::rate_limit` takes a
fixed `requests`/`per_seconds` pair and cannot express a discovered bound, and the one published
figure (a global 50 rps per bot) is shared across routes, so writing it per-operation would state six
allowances no route has. The rule lives in the connector description and in the write's own
description instead, and `the_rate_limit_rule_is_stated_where_a_model_reads_it` asserts both.

**One defect found and fixed in the preserved work.** The prefix-containment assertion scanned the
whole emitted module, including the `description` line — and `discord-current-user`'s description
deliberately names the `Bot ` scheme word, so the test failed on its own prose. The scan now runs over
the emitted *code* with description lines dropped, which is where header assembly would actually
appear.

**Follow-up worth a story:** `permission_overwrites`, `attachments`, `embeds` and guild `roles` are
declared as untyped objects. Enumerating them is real work with real value for a model, and it is not
this story's.

### Rework round 1 — the census was a whole-catalogue assertion in a per-provider test

Review found `the_catalogue_prefix_census_is_exactly_these_four` blocking: it walked `providers/*.toml`
and asserted the result equalled a four-element literal. C-218's Klaviyo declares a fifth prefix
(`Klaviyo-API-Key `), so this file went red from a worktree it could not see — a red that is **not**
among the eight tabulated staleness checks and that no regeneration at integration can resolve,
because it is a hand-written literal in a shipped test. It broke the disjoint-write-set property that
lets provider stories run in parallel.

Reproduced here after merging `main` (old assertion, current tree): `left` 5 entries, `right` 4.

**The fix is not to append Klaviyo** — that reproduces the defect one wave later. The premise being
corrected named *specific connectors*, so the correction names them too:
`the_non_bearer_prefixes_this_connector_joins_were_already_shipped` loads Okta, PagerDuty and
Statuspage **by name** and checks each one's own prefix. The catalogue's *membership* was never the
evidence — what those three declare is. A fifth or fiftieth prefix cannot falsify it; one of those
three changing its scheme word can, which is exactly when the evidence would stop being true. The
model-wide half (`Bearer ` is not spellable as a `Header` prefix) was dropped as duplication: it is
already pinned, fixture-based and growth-proof, at
`crates/connector-spec/tests/auth_prefix.rs::the_preset_schemes_carry_no_prefix_of_their_own`.

A scoping rule is now stated in the test file's module docs: **nothing in a per-provider contract test
walks `providers/`.** Naming a provider is fine; enumerating the directory is not.

**Second finding, non-blocking: the emitted-Flux prefix scan was inert.** `crates/connector-flux/src`
never references `AuthScheme`, so no emitted module can contain a prefix under any declaration — the
scan could not have failed before or after the description-stripping fix. Replaced with
`the_emitter_never_reads_the_credential_declaration`, which emits each operation against a connector
whose credentials have been *removed* and asserts byte-identical output. That pins the actual
invariant ("the emitter is auth-blind") at its cause and fails the moment the emitter reads auth.
Both rewritten tests were mutation-checked to confirm they fail when the property is broken.

**The C-54 guard caught a third thing**, and it was right to.
`shipped_providers_build.rs::no_test_hand_maintains_a_shipped_provider_list` refuses a test `const`
naming two or more shipped providers. The first draft of the fix put the three predecessors in a
`const`; the list now lives in the test body, which is the carve-out that guard's own documentation
names ("a per-provider claim inside a test body … is an assertion about each provider rather than a
copy of the provider set"). It is the correct shape, not an evasion: this list must *not* grow when a
provider is added.

**The ninth is now red, and it is not this story's.** `the_recorded_floor_is_the_measured_figure`
reports coverage 256 of 287 against a floor of 220. Measured with `providers/discord.toml` removed
entirely it is still red — 250 of 281 — so it is the wave accumulating, exactly as `AGENTS.md`
describes. `COVERED_FLOOR` untouched; the coordinator raises it at integration.
