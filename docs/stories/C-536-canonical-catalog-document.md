---
id: C-536
title: "Emit the canonical catalog document per provider"
pillar: Codegen
status: ready
priority: 0
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-cli, connector-spec, artifacts]
note: "One deterministic committed catalog/<name>.catalog.json per provider carrying the complete surface incl. the request template and the four surfaces that reach no artifact today"
---

# Emit the canonical catalog document per provider

## Goal

Lower the IR to one canonical, deterministic, committed JSON document per provider — the reviewed
artifact of Decision 0022 — carrying the complete published surface, including an explicit request
template and the surfaces the op grammar cannot say today.

## Acceptance

- [x] `flux-connectors build` writes `catalog/<name>.catalog.json` for every provider;
      unchanged inputs reproduce it byte for byte, and `diff` covers it like every artifact.
- [x] The document carries: provider/services metadata; every operation with a **request template**
      (method, URL template, parameter placement, body encoding incl. `form`, constant headers,
      endpoint slots) equivalent to what `connector-pack/src/request.rs` derives from the Flux
      body; params/response schemas; the full auth surface including complete `OAuth2Spec` and
      token-endpoint quirks; config fields with bindings; `verify`; events; channel bindings; and
      `quirks.pagination`, `quirks.rate_limit`, `error_envelope` and service `roles` — the
      first artifact any of those four reach.
- [x] The template vocabulary is closed and total: anything `request.rs` refuses today has no
      spelling in the template. A failing-first test proves an unrepresentable construct is a build
      error, never a silently degraded document.
- [x] Each document is hashed per provider in `connectors.lock`; the lockfile invariants
      (byte-stable, no credential, no resolved endpoint) hold — extend
      `tests/lockfile.rs::the_lockfile_carries_no_credential_and_no_endpoint` to the new rows.
- [x] The document carries **no OAuth2 registration value**: `client_id`/`client_secret`/redirect
      URI are per-deployment, published only as the operator-level configuration requirement
      through the existing `binds = "oauth.client_id"` grammar. The vestigial empty `client_id`
      value (`client_id: ""` for gitlab and babelforce in today's generated catalogue) does not
      survive into the document; a provider TOML declaring a non-empty one is a build error, not
      emitted data — and the published document **schema has no field for a registration value at
      all**, so a future document cannot carry one for a consumer to mistakenly trust
      (Exchange-side X-154 additionally ignores any such value; unrepresentable-plus-ignored is
      the pairing, not a promise).
- [x] The document schema is published as a versioned JSON Schema and validated in the build, the
      way `core_catalog.rs` validates `web/public/v1/**`.
- [x] A failing-first differential test proves, for at least one provider, that the request
      template and the Flux-derived request agree field-for-field (the whole-catalogue gate is
      C-538's; this story lands the mechanism).

## Progress

- 2026-08-12: Implemented on `impl/C-536`. The lowering is `crates/connector-cli/src/document.rs`
  (`document::render` + `document::schema`), planned per provider in `pipeline::compile` under the
  new `catalog/` artifact root (C-429 family), with the schema planned on every run; emission is
  additive and no previously emitted artifact moved (`diff` clean at
  `1166 artifacts up to date (55 providers checked)`). The differential mechanism is
  `crates/connector-pack/tests/document_differential.rs` (zendesk, all 3 services, 35 operations,
  path pin included); the closed-vocabulary refusals and the registration-value refusal are unit
  tests in `document.rs`; the artifact/lockfile/schema surface is
  `crates/connector-cli/tests/catalog_document.rs`. `form` bodies are representable and proven
  against a fixture — no shipped provider declares the encoding
  (`grep -rn body_encoding providers/ | grep -v '#'` is empty), so the pack-side form comparison
  first runs when one ships. Graphs are refused, not dropped (design defers their lowering). A
  resuming agent should know: the four-surfaces table in `AGENTS.md` §Intentional gaps was
  re-measured and rewritten (pagination is 4 ops across 2 providers; the old "6 across 3" counted
  comment mentions).

## Notes

- Emission is additive: `.flux` and `.connector.toml` continue to be produced unchanged until
  C-540. No consumer is repointed by this story.
- Write set is `crates/connector-cli` (a new lowering module) plus `crates/connector-spec` only if
  the IR needs accessors; do not share a wave with another story writing either.
