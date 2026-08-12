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

- [ ] `flux-connectors build` writes `catalog/<name>.catalog.json` for every provider;
      unchanged inputs reproduce it byte for byte, and `diff` covers it like every artifact.
- [ ] The document carries: provider/services metadata; every operation with a **request template**
      (method, URL template, parameter placement, body encoding incl. `form`, constant headers,
      endpoint slots) equivalent to what `connector-pack/src/request.rs` derives from the Flux
      body; params/response schemas; the full auth surface including complete `OAuth2Spec` and
      token-endpoint quirks; config fields with bindings; `verify`; events; channel bindings; and
      `quirks.pagination`, `quirks.rate_limit`, `error_envelope` and service `roles` — the
      first artifact any of those four reach.
- [ ] The template vocabulary is closed and total: anything `request.rs` refuses today has no
      spelling in the template. A failing-first test proves an unrepresentable construct is a build
      error, never a silently degraded document.
- [ ] Each document is hashed per provider in `connectors.lock`; the lockfile invariants
      (byte-stable, no credential, no resolved endpoint) hold — extend
      `tests/lockfile.rs::the_lockfile_carries_no_credential_and_no_endpoint` to the new rows.
- [ ] The document carries **no OAuth2 registration value**: `client_id`/`client_secret`/redirect
      URI are per-deployment, published only as the operator-level configuration requirement
      through the existing `binds = "oauth.client_id"` grammar. The vestigial empty `client_id`
      value (`client_id: ""` for gitlab and babelforce in today's generated catalogue) does not
      survive into the document; a provider TOML declaring a non-empty one is a build error, not
      emitted data — and the published document **schema has no field for a registration value at
      all**, so a future document cannot carry one for a consumer to mistakenly trust
      (Exchange-side X-154 additionally ignores any such value; unrepresentable-plus-ignored is
      the pairing, not a promise).
- [ ] The document schema is published as a versioned JSON Schema and validated in the build, the
      way `core_catalog.rs` validates `web/public/v1/**`.
- [ ] A failing-first differential test proves, for at least one provider, that the request
      template and the Flux-derived request agree field-for-field (the whole-catalogue gate is
      C-538's; this story lands the mechanism).

## Progress

- (not started)

## Notes

- Emission is additive: `.flux` and `.connector.toml` continue to be produced unchanged until
  C-540. No consumer is repointed by this story.
- Write set is `crates/connector-cli` (a new lowering module) plus `crates/connector-spec` only if
  the IR needs accessors; do not share a wave with another story writing either.
