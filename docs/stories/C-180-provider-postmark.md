---
id: C-180
title: Ship the Postmark connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "`X-Postmark-Server-Token`, and the server token is per-SERVER while the account token is a different header for a different endpoint set — two credentials scoped to different operation subsets"
---

# Ship the Postmark connector

## Goal

Add Postmark to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**Two credentials, partitioned by operation rather than sent together.** Postmark uses `X-Postmark-Server-Token` for sending and `X-Postmark-Account-Token` for account management. Unlike Datadog they are never sent together — they partition the surface, which is what a *service* is for.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `X-Postmark-Server-Token` (default service) and `X-Postmark-Account-Token` (account service).

**Curated operation set (a starting point, not a mandate):** send an email, get delivery stats, list bounces, get a bounce · account: list servers, get a server

## Hazards specific to this one

This is the clean version of what [C-177](C-177-provider-contentful.md) probes: per-service credentials where the *services* already justify the split. If the credential model is per-service, this connector should be straightforward and is worth landing before Contentful. **Author no recipient addresses.**

## Acceptance

- [x] `providers/postmark.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/postmark.toml`, 6
      operations (4 `server`-service, 2 `account`-service).
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every `[[operations]]` entry in
      `providers/postmark.toml`; `effects` is derived at emission (`effects ["network"]` in every
      `crates/catalog/ops/postmark/*.flux`), not authored, matching every other shipped provider.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → `providers/postmark.toml`'s two `[[config]]` entries (`server_token`, `account_token`), each
      `secret = true` and bound via `credential.postmark.*`; enforced by the loader
      (`crates/connector-spec/src/config.rs`'s `Binding::is_secret` agreement rule) and green in the
      gate.
- [x] A `verify` operation that is a read and runs unattended. → `verify = "postmark-deliverystats-get"`
      — `GET /deliverystats`, `risk = "low"`, `idempotency = "idempotent"`, no required argument.
- [x] `crates/connector-flux/tests/postmark_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. → 5 tests:
      two credentials on independent headers/env vars; every operation's `effective_auth` resolves to
      exactly one credential matching its own service and never both; `credential_ref_for` measured
      still eliding the service segment for both credentials (see `## Progress`); every operation
      emits; no email-address-shaped literal anywhere in the provider file.
- [x] **Failing-first test:** the contract test must fail before `providers/postmark.toml` exists. → see
      `BASE_PROOF` in the handoff report; all 5 tests failed (file-not-found) with `postmark.toml`
      absent, all 5 pass with it present.
- [x] The scoped gate is green: `build --provider postmark`, `diff --provider postmark` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. → all green;
      `diff --provider postmark` reports "11 artifacts up to date (1 provider checked)".
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly the documented
      eight, plus the documented ninth (`the_recorded_floor_is_the_measured_figure`); no tenth, despite
      this being a two-service provider. See `## Progress`.

## Progress

- **The credential-addressing question this story asked ("confirm it against `CredentialRef`") holds,
  but one level down from where the story note points.** `CredentialRef::new`
  (`crates/connector-spec/src/credential.rs`) genuinely accepts an arbitrary `service` argument — the
  headroom is real. But `Connector::credential_ref_for` (`crates/connector-spec/src/ir.rs:1166-1178`),
  the function that actually turns a *declared* credential into a tenant path, always renders the
  reserved default service for every credential, regardless of which service its operations belong to
  — its own doc comment (`ir.rs:1153-1160`) already says this is deliberate, because credential *names*
  are unique within a provider and already distinguish the case. Postmark does not change that:
  `postmark.server_token` and `postmark.account_token` still both resolve through `credential_ref_for`
  to a 4-segment path under the elided default service, distinguished only by leaf name. Measured by
  `credential_ref_for_elides_the_service_and_the_two_tokens_still_never_collide` in
  `crates/connector-flux/tests/postmark_connector.rs`.
- **What actually keeps the two tokens from ever being sent on the same request is a different,
  already-shipped mechanism**: per-operation `auth` (`Operation::auth`, `ir.rs:652-669`) overriding
  `Connector::default_auth` — the same override babelforce's own `[[patch.operations]] auth = [...]`
  overlay entry already exercises on a different axis. **No change to `connector-spec` was needed to
  ship this connector.** For C-177 (Contentful): the takeaway is that a service split needs only
  distinct credential names plus a per-operation `auth` override, not a change to `credential_ref_for`
  or `CredentialRef` — recommend against adding a per-credential `service` field to `credential_ref_for`
  unless a future connector actually needs two *same-named* credentials disambiguated only by service,
  which nothing shipped today does.
  This is confirmed directly in the generated artifact, not merely by the source declaration:
  `crates/catalog/src/generated/postmark.rs` shows the two `account`-service operations carrying
  `credentials: &[&["postmark.account_token"]]` and all four `server` operations carrying
  `postmark.server_token`, with none carrying both — the partition is enforced at the only level any
  shipped connector's auth exists at today.
- **REWORK (independent review): the in-band credential hazard was disclosed nowhere a reader
  reaches.** The first pass documented `GET /servers`'/`GET /servers/{id}`'s real `ApiTokens` field (a
  server's own live Server Token, returned in plaintext by the Account API) only in the TOML's header
  comment, then explicitly omitted the property from `response_schema` with "Not declared here." That
  is the wrong convention: `response_schema` is cloned into the published site catalogue
  (`crates/connector-cli/src/site.rs`) and is the tool contract a model reads before calling — a source
  comment reaches neither. Fixed by following `providers/zoom.toml`'s `start_url` /
  `providers/zendesk.toml`'s `authenticity_token` convention instead: both operations now declare
  `ApiTokens`' shape (`array` of `string`, no example) with a description stating it is
  "ACCOUNT-PRIVILEGED" live credential material and must not be logged, echoed, or passed to another
  tool, and both operations' own top-level `description` says so too, since that is the text a model
  reads before deciding to call. Re-ran the full scoped gate afterward: `build --provider postmark` and
  `diff --provider postmark` are clean, and the red count is unchanged — exactly the same documented
  eight plus the ninth (`the_recorded_floor_is_the_measured_figure`; postmark still 6/6 in the coverage
  breakdown).
- **A second, unanticipated finding surfaced while authoring**: declaring the `account` service at all
  removes the implicit `default` service for *every* operation in the provider (`AGENTS.md`'s service
  contract, enforced by the loader's `validate_member_service`/`validate_operation_service`). The four
  Server Token operations therefore needed an explicit second named service, `server`, rather than
  staying elided — this is a narrower, separate rule from the credential-addressing question, and it is
  why `providers/postmark.toml` declares two `[[services]]` entries instead of one.
- **Unverified against the live API** (no vendor spec is vendored for Postmark; see the TOML's header
  comment): the exact set of fields Postmark accepts/returns beyond what is declared here. In
  particular, `Subject`/`ReplyTo`/`Tag` on `postmark-email-send` are declared optional based on
  Postmark's published reference prose, not a machine-checked spec; and `GET /servers`'s real payload
  additionally carries `ApiTokens` (the server's own Server Token, returned in-band), which this
  connector's `response_schema` deliberately does not declare — see the TOML's header comment.
- Nothing was invented: every operation and field name here is either directly from Postmark's own
  reference documentation naming (`From`, `To`, `Subject`, `HtmlBody`, `TextBody`, `ErrorCode`,
  `Message`, `TotalCount`, `Bounces`, `Servers`, etc.) or a `snake_case` rendering of it via `wire`.

## Notes

- **Charter fit.** Postmark is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
  non-goals exclude technology adapters and the inference path. If authoring reveals it is really a
  technology rather than a service, stop and say so rather than shipping it.
- **Provenance is hand-authored and drift is undetectable by machine**, the caveat zendesk, freshdesk,
  github and notion already carry. Record it in the TOML's header comment.
- **Do not invent an endpoint.** A confident set of four operations beats a guessed set of ten — the
  repository's rule is that a loud refusal beats plausible-but-incorrect output, and a hallucinated
  path ships as a `404` a contract test cannot catch. Where you are unsure of a path, shape or required
  field, leave the operation out and name it in `## Progress` as unverified.
- **No credential value, and no realistic-looking `example` on a secret field.** A placeholder shaped
  like a real token has already tripped GitHub's push protection and blocked a release.
- Whole-catalogue artifacts are coordinator-owned: `crates/catalog/src/generated.rs`,
  `web/public/catalog.json`, `web/public/v1/**`, `assets/readme-snippet-*.svg`. The per-provider
  `crates/catalog/src/generated/postmark.rs` is **not** in that set and is yours to commit.

### Coordinator note at integration

**Integrated after one REWORK round, on a finding an independent review caught and neither the
implementor nor the coordinator would have.**

The connector originally noted that `GET /servers` returns `ApiTokens` — live server tokens in
plaintext — and then *omitted* the property from `response_schema` with "Not declared here." That reads
as caution and is the opposite. `crates/connector-cli/src/site.rs:680` clones `response_schema` into
`web/public/catalog.json`, which `providers/zoom.toml` itself calls *"the one place a consumer will
read"*, so the only hazard note lived in a TOML source comment reaching no artifact — leaving a
published schema that looks complete over a response that embeds credential material.

`providers/zoom.toml`'s `start_url` and `providers/zendesk.toml`'s `authenticity_token` had already
settled the convention: keep the field **in** the schema with a description that states the danger, so
the warning travels with the data. Declaring a shape and warning about it needs neither an example nor a
value, so this never touched the credential-value invariant.

Verified at integration rather than accepted on report: the disclosure appears in both operations'
`response_schema` **and** their top-level `description` (the text a model reads before calling), reaches
`crates/catalog/src/generated/postmark.rs`, and now appears six times in the published
`web/public/catalog.json`.

The review also corrected the framing in the implementor's own report, which is recorded on
[C-122](C-122-provider-anthropic.md): the two-token partition is enforced by `Operation::auth`, not by
`credential_ref_for`.
