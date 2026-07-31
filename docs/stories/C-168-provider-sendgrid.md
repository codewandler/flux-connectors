---
id: C-168
title: Ship the SendGrid connector
pillar: Spec
status: in-progress
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "the send body is a nested array-of-objects envelope (`personalizations[].to[]`) — deeper than Asana's envelope, and the first place body nesting depth is really tested"
---

# Ship the SendGrid connector

## Goal

Add SendGrid to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A deeply nested send envelope.** `POST /v3/mail/send` takes `personalizations: [{to: [{email}]}]` — an array of objects containing arrays of objects.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <api_key>`.

**Curated operation set (a starting point, not a mandate):** send a mail, list templates, get a template, list suppressions, validate an email address

## Hazards specific to this one

Check what `body` nesting the schema expresses before promising the send operation; C-144 records that a nested field is refused under `form` encoding, and the body-path refusals in `AGENTS.md` name *a nested body path without a wrapper*. If the envelope is not expressible, ship the read operations and record the send as the gap. **Never author a recipient address** — no personal data in this repository.

## Acceptance

- [x] `providers/sendgrid.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. Four ops:
      `sendgrid-template-list`, `sendgrid-template-get`, `sendgrid-suppression-bounce-list`,
      `sendgrid-email-validate`. `sendgrid-mail-send` is excluded — see the header comment and
      `## Progress` below for why.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. `effects` is emitter-derived (`["network"]`), not
      author-declared; `risk`/`idempotency` are declared on all four operations.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      One field, `api_key`, `secret = true`, `binds = "credential.sendgrid.api_key"`.
- [x] A `verify` operation that is a read and runs unattended. `verify = "sendgrid-template-list"`, a
      `GET` with no required argument, `risk = "low"`.
- [x] `crates/connector-flux/tests/sendgrid_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. The central
      assertion (`a_wire_path_that_looks_like_an_array_index_still_builds_an_object`) is mechanical: it
      builds a synthetic fixture with `wire = "personalizations.0.to.0.email"` and shows the emitter
      does **not** refuse it — it silently assembles nested objects with quoted numeric keys
      (`{ personalizations: { "0": { to: { "0": { email: to_address } } } } }`), never a JSON array,
      which is the mechanical proof that this pipeline's `wire`/`BodyNode` mechanism has no array
      primitive at any depth.
- [x] **Failing-first test:** the contract test must fail before `providers/sendgrid.toml` exists. See
      `BASE_PROOF` in the implementation report — 8 of 9 tests fail at the merge base (the ninth, the
      synthetic-fixture archetype test, is deliberately independent of the provider file, since its
      claim is about the pipeline mechanism rather than this one connector).
- [x] The scoped gate is green: `build --provider sendgrid`, `diff --provider sendgrid` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. All green
      except the expected whole-catalogue staleness (next item).
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. **A ninth is also red**:
      `the_recorded_floor_is_the_measured_figure` (coverage 120/138 against a floor of 105, per
      AGENTS.md's documented two-way ratchet). Not edited — the coordinator raises it at integration.

## Notes

- **Charter fit.** SendGrid is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/sendgrid.rs` is **not** in that set and is yours to commit.

## Progress

**The send operation is excluded, deliberately.** `crates/connector-flux/src/op.rs`'s `BodyNode` (the
type `wire` paths assemble into) has exactly two variants — `Leaf` and `Branch(BTreeMap<String,
BodyNode>)` — so every `wire` segment becomes an object key, at every depth, with no array primitive
anywhere in the mechanism. SendGrid's `personalizations: [{"to": [{"email": …}]}]` needs two literal
JSON arrays (SendGrid rejects the bare-object shorthand), and no named body field can be placed
*inside* an array through `wire`. The one remaining route — a single body-root parameter whose own
declared value is `{type: "array", ...}` — is mechanically legal (`check_body_encoding` only refuses
an object/array-valued field under `form` encoding, and this connector is `json`), but it decomposes
nothing: the caller would have to construct the entire nested envelope itself with no structural help
from this pipeline, which is exactly the "ships an untyped blob and hands the model a guess at the
shape" pattern `providers/notion.toml` already refused for its recursive block model. Full reasoning
and the citation trail are in `providers/sendgrid.toml`'s header comment, and
`crates/connector-flux/tests/sendgrid_connector.rs`'s
`a_wire_path_that_looks_like_an_array_index_still_builds_an_object` makes it mechanical rather than
asserted-only.

**Unverified / not independently confirmed against a live account** — flagged rather than guessed
past: the exact response envelope shape for `GET /v3/templates` (`result` + `_metadata` cursor
pagination) and the exact response shape of `POST /v3/validations/email` (a `result` object carrying
`verdict`/`score`/`local`/`host`/`suggestion`) are recalled from SendGrid's published API reference
rather than fetched live — this repository has no vendored OpenAPI document for SendGrid to check
either against (C-4/C-14 are both unimplemented). Both are declared loosely (few required fields, no
over-specific nested schemas on fields I'm not confident of) for exactly this reason. The Bounces API
(`GET /v3/suppression/bounces`) and the Templates single-get (`GET /v3/templates/{template_id}`) are
long-stable, frequently-documented SendGrid endpoints I have higher confidence in.

**Which of SendGrid's several suppression lists this ships:** only Bounces
(`sendgrid-suppression-bounce-list`). SendGrid also has Blocks, Invalid Emails, Spam Reports and
Global (Unsubscribe) suppression lists, each its own endpoint with its own response shape; only
Bounces is selected here rather than guessing at the other three's exact fields.
