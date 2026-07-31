---
id: C-169
title: Ship the Cloudflare connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "zone-scoped paths where the zone id is a required first segment on every operation — a natural test of whether `[[config]]` can pin a path variable rather than a caller supplying it each call"
---

# Ship the Cloudflare connector

## Goal

Add Cloudflare to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A tenant id in every path.** Nearly every Cloudflare endpoint is `/zones/{zone_id}/…`. Whether that is configuration or a per-call argument is a design decision, not a detail.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <api_token>`.

**Curated operation set (a starting point, not a mandate):** list DNS records, create a DNS record, delete a DNS record (destructive), purge cache, list zones

## Hazards specific to this one

`purge cache` and `delete a DNS record` are the operations whose declared risk and idempotency matter most — a cache purge is idempotent but expensive, a DNS delete is destructive. Get those declarations right; they are what the tool-contract gate reads.

## Acceptance

- [x] `providers/cloudflare.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/cloudflare.toml`,
      5 operations (list zones, list/create/delete DNS records, purge cache).
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every `[[operations]]` block in
      `providers/cloudflare.toml`; `effects` is derived by the emitter (`["network"]` in
      `connectors/cloudflare.flux`), not authored.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → `providers/cloudflare.toml`'s single `[[config]]` block (`api_token`); the loader itself
      enforces the `secret`/`binds` agreement (`crates/connector-spec/src/config.rs`).
- [x] A `verify` operation that is a read and runs unattended. → `verify = "cloudflare-zone-list"`,
      a parameter-free `GET`, `risk = "low"`; asserted by
      `the_connector_verifies_with_a_read_over_a_bearer_token`.
- [x] `crates/connector-flux/tests/cloudflare_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. → 11 tests;
      `the_zone_id_is_a_per_call_argument_everywhere_but_zone_list` and `no_config_field_binds_a_zone`
      are the archetype tests, `the_dns_record_delete_is_destructive_and_not_claimed_idempotent` and
      `the_cache_purge_is_high_risk_and_forced_non_idempotent_by_the_post_rule` are the hazard tests.
- [x] **Failing-first test:** the contract test must fail before `providers/cloudflare.toml` exists. →
      proved at `$(git merge-base main HEAD)` with `providers/cloudflare.toml` absent: all 11 tests
      fail with "cannot read … providers/cloudflare.toml".
- [x] The scoped gate is green: `build --provider cloudflare`, `diff --provider cloudflare` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. → all green
      except the nine tests named below, which are expected.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly the eight
      AGENTS.md tabulates, confirmed by name (see `## Progress`). A **ninth**,
      `the_recorded_floor_is_the_measured_figure`, is also red — expected per AGENTS.md's own note
      that this check is red per *wave*, and this connector's five operations are 5/5 covered by
      `response_schema`, which is what tips the aggregate over `COVERED_FLOOR`. Not edited, per the
      dispatch.

## Notes

- **Charter fit.** Cloudflare is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/cloudflare.rs` is **not** in that set and is yours to commit.

## Progress

**The design decision, answered by the schema rather than by preference.** `zone_id` is a per-call
`params.path` argument on every operation except `cloudflare-zone-list` (the call that discovers zone
ids and has nothing yet to scope). It could not have been `[[config]]`: `ConfigField::binds` /
`parse_binding` (`crates/connector-spec/src/config.rs`) only ever reaches an `endpoint.<variable>`
placeholder in `Connector::base_url`, and Cloudflare's `base_url` is one host
(`https://api.cloudflare.com/client/v4`) shared by every zone, with no zone-shaped placeholder to
bind. So this is not a case where either shape was viable and one was chosen for its properties — the
schema has no vocabulary to express the configuration alternative at all. The full reasoning, including
the named consequence (a model holding the token can address any zone the *token* can reach, not only
"the" zone an operator meant to install it against, bounded by Cloudflare's own token-scoping UI), is
in `providers/cloudflare.toml`'s header comment and is asserted by
`the_zone_id_is_a_per_call_argument_everywhere_but_zone_list` and `no_config_field_binds_a_zone` in the
contract test.

**The two hazard declarations.** `cloudflare-dns-record-delete` is `risk = "destructive"`,
`idempotency = "non_idempotent"` (Cloudflare answers a repeat with 404, not a repeat 200, and
documents no idempotency guarantee — the same call `providers/airtable.toml` makes for its own
delete). `cloudflare-cache-purge` is `risk = "high"` (instantaneous, zone-wide, externally visible, can
spike origin load — not `destructive`, since the cache repopulates from origin) and, more subtly,
`idempotency = "non_idempotent"` **despite being genuinely idempotent by Cloudflare's own behaviour**:
`crates/connector-flux/src/op.rs`'s `check_write_metadata` refuses `idempotency = "idempotent"` on any
`POST` outright, so the honest declaration cannot be expressed and the loss of fidelity is recorded in
the provider file's comment rather than absorbed silently — the same trade `providers/notion.toml`
already carries for its two `POST` reads. `the_cache_purge_is_high_risk_and_forced_non_idempotent_by_the_post_rule`
proves the refusal directly, by cloning the connector, flipping the declared idempotency to
`Idempotent`, and asserting `emit_operation` now errs.

**Confidence and what was deliberately left out.** The five shipped operations
(`GET /zones`, `GET /zones/{zone_id}/dns_records`, `POST /zones/{zone_id}/dns_records`,
`DELETE /zones/{zone_id}/dns_records/{dns_record_id}`, `POST /zones/{zone_id}/purge_cache`), their
methods, Cloudflare's `{success, errors, messages, result}` envelope, the 32-character lowercase-hex
id shape, and the bearer API-token auth are all high-confidence — this is one of the most-documented,
longest-stable HTTP APIs in the ecosystem. Left out, each named in the provider file's header comment
rather than silently absorbed:
- **DNS record read-one and update** (`GET`/`PATCH`/`PUT .../dns_records/{dns_record_id}`) — not in
  the curated set; a confident, small follow-on rather than a gap.
- **List pagination and filters** (`page`, `per_page`, `name`, `type`, …) — not a query-encoding
  problem (integers and enums need no percent-encoding), but this file is not confident of the exact
  `per_page` bounds per endpoint and would rather return Cloudflare's first page than assert a cap
  that might be wrong. Named as the one place a future author should verify a vendor-documented number
  before adding it, rather than as something this file got wrong.
- **Selective cache purge** (`files`/`tags`/`hosts` in the purge body) — `tags`/`hosts` are
  Enterprise-plan-only, and mixing them with `purge_everything` in one body is the ambiguous
  free-form shape AGENTS.md refuses; `cloudflare-cache-purge` does the one thing every plan supports.
- Optional DNS-record-create fields (`ttl`, `proxied`, `comment`, `tags`) — excluded per the C-56
  null-on-omission gap, the same call `providers/shopify.toml` and `providers/airtable.toml` make.

Nothing above is guessed at a level this file is not confident of; where confidence ran out, the
operation or field was left out rather than shipped as a plausible guess.
