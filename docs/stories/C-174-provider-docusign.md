---
id: C-174
title: Ship the DocuSign connector
pillar: Spec
status: in-progress
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "the base URI is returned by a userinfo call and is per-account (`{base_uri}/restapi/v2.1/accounts/{account_id}`) — TWO configured path levels above every operation"
---

# Ship the DocuSign connector

## Goal

Add DocuSign to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A two-level configured prefix.** DocuSign operations sit under a per-account base URI *and* an account id, both discovered rather than constant.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** OAuth2 bearer access token.

**Curated operation set (a starting point, not a mandate):** list envelopes, get an envelope, get envelope recipients, create an envelope from a template, void an envelope (destructive)

## Hazards specific to this one

Shares the configured-host question with [C-163](C-163-provider-salesforce.md) and [C-164](C-164-provider-algolia.md) — read whichever landed first rather than re-deriving it. An envelope is a legal signature request: `void an envelope` is destructive and `create an envelope` has real-world effect, so the declared effects matter more here than anywhere else in this fleet. **Signer names and emails are personal data — author none.**

## Acceptance

- [x] `providers/docusign.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. Six operations: `docusign-verify`,
      `docusign-envelope-list`, `docusign-envelope-get`, `docusign-envelope-recipients-get`,
      `docusign-envelope-create-from-template`, `docusign-envelope-void`.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
- [x] A `verify` operation that is a read and runs unattended (`docusign-verify`, `GET /folders`).
- [x] `crates/connector-flux/tests/docusign_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about: the two-level configured host as two independently-bound
      `base_url` variables (`the_base_url_carries_two_independently_bound_variables`), plus the PII,
      query-encoding and array-decomposition hazards below.
- [x] **Failing-first test:** the contract test must fail before `providers/docusign.toml` exists. See
      `BASE_PROOF` in the implementation report — all 8 tests failed at the merge base with "cannot
      read providers/docusign.toml".
- [x] The scoped gate is green: `build --provider docusign`, `diff --provider docusign` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. Measured: exactly the eight AGENTS.md names, across the five named
      binaries, no more, no fewer. A **ninth**, `the_recorded_floor_is_the_measured_figure`
      (`response_schema_coverage.rs`), is also red — coverage climbed to 188/214 against a floor of
      166, comfortably inside the ratchet's own one-tenth slack per AGENTS.md's *"per wave, not per
      story"* rule, and it is coordinator-owned (raises `COVERED_FLOOR`, a fenced constant), so it is
      reported here and left untouched rather than edited.

## Progress

- **The configured-host decision: fold both variables into `base_url`, not a per-call `account_id`
  argument.** C-163/C-164 (whichever landed first) already answered "yes, `endpoint.<variable>` can
  pin a per-tenant host"; this story's own hazard is that DocuSign needs *two* such facts (the account
  host and the account id), and C-169/C-170 (filed as C-187) measured that `binds` reaches `base_url`
  and nothing else — no path segment, no query parameter. Read literally, that sounds like account id
  (a path segment in DocuSign's own URL shape) is out of `[[config]]`'s reach and has to be a per-call
  argument instead. It is not, once `crates/connector-spec/src/config.rs::template_variables`
  (`config.rs:348-362`) is read directly rather than assumed: it extracts *every* `{...}` placeholder
  in a `base_url`, and `provider.rs`'s `validate_binding` (`provider.rs:557-573`) and
  `validate_every_template_variable_is_asked_for` (`provider.rs:638-660`) check — and require a bound
  field for — each one individually, with no cap on how many a `base_url` carries. So
  `base_url = "https://{account_host}/restapi/v2.1/accounts/{account_id}"` with two `[[config]]`
  fields (`binds = "endpoint.account_host"`, `binds = "endpoint.account_id"`) is fully expressible
  today, verified by loading the connector and asserting on it directly
  (`the_base_url_carries_two_independently_bound_variables`) rather than trusting the reasoning. This
  is the cleaner answer of the two the story offered: DocuSign's account id is a per-connection
  constant discovered once (the same shape Salesforce's `instance` already has), not a per-call choice
  a model should ever be asked to supply and could get wrong. Nothing here needed C-187 to land, and
  nothing here disagrees with what C-187 measured — account id never becomes a path segment in this
  connector's own operations, because it never leaves `base_url`.
- **C-186 does not bind here.** This connector declares no `PATCH` operation, and its one `POST`
  (`docusign-envelope-create-from-template`) is genuinely `non_idempotent` — calling it twice creates
  two envelopes — so there is no case where `check_write_metadata`'s POST/PATCH idempotent-refusal
  (`crates/connector-flux/src/op.rs:594-604`) forces a declaration this file believes is wrong. Void is
  a `PUT`, which the same check allows to be idempotent; it is declared `non_idempotent` anyway,
  conservatively, following `providers/miro.toml`'s sticky-note-delete and `providers/cloudflare.toml`'s
  DNS-record-delete precedent: this file has no confirmed evidence DocuSign answers a *repeated* void
  with the same success rather than an "already voided" error, so the safe under-claim is what ships.
- **Excluded, deliberately: DocuSign's "Create Recipient View" (embedded signing URL).** This is
  exactly the "signing URL that acts as a bearer token" hazard the story calls out — a URL that starts
  an embedded signing session as a specific recipient with no further authentication. Unlike Zoom's
  `start_url`, which two shipped Zoom operations return whether this connector likes it or not and so
  must declare with its hazard stated, nothing in this connector's curated set calls DocuSign's
  recipient-view endpoint, so there is no field to declare or omit — it is left out of the operation
  set entirely rather than shipped with a fabricated recipient identity to construct a request with.
- **Excluded, unverified: a `status` query filter on `docusign-envelope-list`, and `to_date`.** Real
  DocuSign query filters beyond `from_date` almost certainly exist, but this story asked for four
  confident operations over ten guessed ones, and `from_date` alone is enough to make the list
  operation honestly callable. Left out rather than guessed at.
- **Excluded, unverified: document download/upload.** DocuSign's document-content endpoints are binary
  (PDF) transfer, a different shape from every other operation here, and this connector does not touch
  it.
- **`docusign-envelope-get`'s and `docusign-envelope-void`'s exact response envelopes are a moderate,
  not certain, recollection** of DocuSign's REST API reference — no vendor spec was available to
  cross-check against (see the TOML's own header comment on provenance). Where confidence was lower
  (a `status` query filter, embedded signing, document content) the operation was left out entirely
  rather than shipped on a guess, per the story's own instruction.

## Notes

- **Charter fit.** DocuSign is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/docusign.rs` is **not** in that set and is yours to commit.
