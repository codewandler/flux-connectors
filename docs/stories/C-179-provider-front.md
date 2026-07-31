---
id: C-179
title: Ship the Front connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "resource ids carry a TYPE PREFIX (`cnv_`, `msg_`, `tea_`) that a model must not invent, and pagination is a full-URL `_links.next`"
---

# Ship the Front connector

## Goal

Add Front to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**Prefixed ids and link-based pagination.** Front's ids are `cnv_55c8c149`; its next page is an absolute URL in `_links.next`. Neither is guessable, and the second cannot be expressed as a cursor parameter.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <api_token>`.

**Curated operation set (a starting point, not a mandate):** list conversations, get a conversation, list messages in a conversation, send a reply, add a tag

## Hazards specific to this one

If pagination is an opaque next-URL rather than a parameter, say that page 2 is unreachable in this pipeline rather than inventing a parameter for it — an operation that silently only ever returns page 1 is the plausible-but-wrong output `AGENTS.md` forbids. **Conversation content is customer data; author no example bodies.**

## Acceptance

- [x] `providers/front.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. Six operations: `front-verify`,
      `front-conversation-list`, `front-conversation-get`, `front-conversation-message-list`,
      `front-conversation-reply`, `front-conversation-tag-add`.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
- [x] A `verify` operation that is a read and runs unattended (`front-verify`, `GET /conversations?limit=1`).
- [x] `crates/connector-flux/tests/front_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (prefixed ids said twice; pagination said unreachable), not that
      the file parses.
- [x] **Failing-first test:** the contract test must fail before `providers/front.toml` exists. See
      `BASE_PROOF` in the implementation report — all 9 tests failed at the merge base with "cannot
      read providers/front.toml".
- [x] The scoped gate is green: `build --provider front`, `diff --provider front` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. Measured: exactly the eight AGENTS.md names, no more, no fewer.
      `the_recorded_floor_is_the_measured_figure` stayed green (this story's coverage addition fit
      inside the floor's existing slack).

## Progress

- **Front does have a pagination mechanism a caller can name** — `page_token`, documented in Front's
  own OpenAPI parameter table — so this is the Notion/Slack/Asana opaque-cursor shape, not a case with
  no mechanism at all. It is excluded for the same reason Notion's `start_cursor` is: unconstructable
  by a caller, and unencodable regardless because nothing here percent-encodes a query value (C-30).
  The response-side vehicle is `_pagination.next` (a full absolute URL), which the story's own note
  paraphrased as `_links.next` — verified against `dev.frontapp.com`'s reference documentation directly
  (fetched, not assumed); `_links` is a real but different field (`_links.self` only).
- **Investigated whether the reply operation's excluded fields trigger C-185** (arrays via `BodyNode`),
  per this story's hazard note. Read `body_tree`/`BodyNode` in `crates/connector-flux/src/op.rs`
  directly: it refuses only an array a wire path would have to *decompose* across nested segments: a
  flat, single-level array (Front's `to`/`cc`/`bcc`, all documented as plain handle-string arrays, and
  this connector's own `tag_ids`) is one Flux argument and hits no such limitation — confirmed by the
  emitted Flux, which declares `tag_ids: List<String>` and assembles `payload = { tag_ids }` cleanly.
  The actual reason `to`/`cc`/`bcc`/`channel_id`/`author_id`/`subject`/`text`/`options`/`attachments`
  are absent from `front-conversation-reply` is C-56 (every one is optional in Front's own schema, and
  this pipeline cannot omit an optional body field without sending an explicit `null`), not C-185.
  Recorded in `providers/front.toml`'s header comment so a future reader does not have to re-derive it.
- **`front-conversation-reply`'s actual response is a 202-Accepted queuing acknowledgement**
  (`{"status": "accepted", "message_uid": ...}`), not the sent message — verified against Front's
  reference documentation for `create-message-reply`. `response_schema` and `description` say so.
- **Unverified / not shipped:** `POST /channels/{channel_id}/messages` (compose a new outbound
  message, as opposed to replying to an existing conversation) — this connector never lists channels,
  and the endpoint's recipient shape was not independently verified, so it is left out rather than
  guessed at. A `GET /me` "who am I" endpoint exists in Front's OAuth flow but is documented as
  identifying the *company* that completed an OAuth grant rather than a plain API token's owner, so it
  is not used as `verify`; `front-verify` (a bounded `GET /conversations`) is used instead, mirroring
  `freshdesk-test`'s own precedent.

## Notes

- **Charter fit.** Front is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/front.rs` is **not** in that set and is yours to commit.
