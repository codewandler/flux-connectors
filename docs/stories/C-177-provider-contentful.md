---
id: C-177
title: Ship the Contentful connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "space AND environment are both path variables, and there are two distinct hosts (delivery vs management) with different credentials — the first provider whose SERVICES need different authorities and different secrets"
---

# Ship the Contentful connector

## Goal

Add Contentful to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**Two hosts, two credentials, one vendor.** Contentful's delivery API (`cdn.contentful.com`) and management API (`api.contentful.com`) are different authorities with different tokens. The service contract says a service owns its base URL; this is the first time it also needs to own its credential.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** Two bearer tokens, one per service: a delivery token and a management token.

**Curated operation set (a starting point, not a mandate):** delivery: get an entry, list entries, get an asset · management: create an entry, publish an entry

## Hazards specific to this one

Read the credential-addressing contract before writing: a credential path is `pid` + service, which suggests per-service credentials are *already* addressable — confirm that against `CredentialRef` rather than assuming. If it holds, this connector is the proof; if it does not, it is the finding. Do **not** pair this with an inbound surface (see [C-173](C-173-provider-typeform.md)'s note).

## Acceptance

- [x] `providers/contentful.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/contentful.toml`, 5
      operations (delivery: get an entry, list entries, get an asset · management: create an entry,
      publish an entry).
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every `[[operations]]` block in `providers/contentful.toml`.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → six `[[config]]` fields (two credentials, four `endpoint.*` fields split per service — see the
      TOML header comment for why the count is four, not two).
- [x] A `verify` operation that is a read and runs unattended. → `verify = "contentful-entries-list"`,
      no required parameter at all; `crates/connector-flux/tests/contentful_connector.rs`'s
      `verify_is_argument_free` asserts it directly.
- [x] `crates/connector-flux/tests/contentful_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. → 5 tests:
      per-service base URL + config ownership, per-operation auth resolution, verify's argument-freedom,
      every operation emitting (including the free-form-body create), and the write risk/idempotency
      rules.
- [x] **Failing-first test:** the contract test must fail before `providers/contentful.toml` exists. →
      proved at merge-base `54cfe164d6cd7f2ebf97d1dea18a1863097d7f82`; see `BASE_PROOF` in the handoff
      report.
- [x] The scoped gate is green: `build --provider contentful`, `diff --provider contentful` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly the eight
      `AGENTS.md` names, across the same five binaries, no ninth.

## Progress

**The credential question was pre-answered (per dispatch) and confirmed, not re-derived**: two bearer
tokens, partitioned by service, via `Operation::auth` overriding `default_auth`
(`crates/connector-spec/src/ir.rs:652-669`) — the same mechanism `providers/postmark.toml` shipped.
`contentful.delivery_token` is the connector default (lower privilege); both management operations
override `auth` explicitly.

**What this story actually forces, per its own note, is the path half**: `space_id`/`environment_id`
are modelled as **service `base_url` template variables** (`endpoint.<var>` config bindings), not as
per-operation path parameters the way `providers/airtable.toml`'s `baseId`/`tableId` are — because a
Contentful token is provisioned against one known space, a property of the *installation* rather than
of an individual call (contrast Airtable's base/table, which genuinely differ call to call). This is
the first provider whose service `base_url` carries two template variables, and the first where two
services each need their own copy: since `ConfigField::name` is unique across the whole connector, not
per service (`validate_config`, `crates/connector-spec/src/provider.rs:481-497` — measured directly by
first attempting shared names `space_id`/`environment_id` and hitting the loader's duplicate-name
refusal), the four resulting fields are named `delivery_space_id`/`delivery_environment_id` and
`management_space_id`/`management_environment_id`. This choice is also what makes `verify`
(`contentful-entries-list`) genuinely argument-free: space/environment are already resolved from
configuration, so the test operation needs no entry or asset id a settings page would have to invent.

`contentful-entry-create`'s body is declared with a free-form `body_schema` (`{"fields": {...}}`),
not named `params.body` fields — Contentful's field ids and locale keys are the space's own content
model and are not knowable at build time (the same reasoning `providers/airtable.toml`'s `fields` and
`providers/babelforce.toml`'s session-variable bodies already carry, C-29). Consequence: this
operation never reaches `BodyNode` at all, so the never-arrays gap (C-168/C-185) is sidestepped by
construction rather than exercised — noted rather than silenced, since a real Contentful field can
be array-typed.

**Unverified / lower confidence, named rather than guessed**: the exact response shapes of
`contentful-asset-get`'s `fields.file` (nested `details`/`image` sub-objects) and the envelope fields
on `contentful-entries-list` (`sys.type`, `skip`, `limit`) are described from general knowledge of the
vendor's documented shape rather than from a vendored spec — flagged in `response_schema` descriptions
as "vendor-defined" / described rather than exhaustively enumerated, following the convention
`providers/anthropic.toml` and `providers/airtable.toml` use for the same kind of volatile or
per-tenant shape. No path, method or required field was guessed at; where confidence was insufficient
(e.g. Contentful's full query-filter surface — `content_type`, `select`, full-text `query`) the
parameter was left out rather than included speculatively.

## Notes

- **Charter fit.** Contentful is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/contentful.rs` is **not** in that set and is yours to commit.
