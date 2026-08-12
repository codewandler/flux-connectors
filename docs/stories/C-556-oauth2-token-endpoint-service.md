---
id: C-556
title: "An OAuth2 declaration may place its token endpoint on a second service"
pillar: Codegen
status: in-progress
priority: 1
epic: catalog-artifact
areas: [connector-spec, connector-cli]
note: "C-555's measured gap: OAuth2Spec binds authorize and token to ONE declared service, so Anthropic's subscription flow (authorize on claude.ai, token on platform.claude.com) is inexpressible. The fix is an optional second service REFERENCE — a name, never a URL, so http_hosts and the declared-authority rules keep working"
---

# An OAuth2 declaration may place its token endpoint on a second service

## Goal

`OAuth2Spec` gains an optional `token_endpoint` — the declared name of a second service whose base
URL the `token_path` resolves against, defaulting to the existing single `endpoint` when absent.
A name, never a URL: the host set stays derived from declared services, so `http_hosts`,
declared-authority validation and X-154's `NoDeclaredDefault` composition rule all keep working
unchanged. This is the loader/spec extension C-555 stopped at, specified from its finding.

## Acceptance

- [x] `OAuth2Spec` carries `token_endpoint: String` (the loader's idiomatic optional, mirroring
      `endpoint`), validated: it must name a declared service of the same
      connector, and a dangling name is a loud loader refusal
      (`provider.rs::validate_one_credential_token_endpoint`). Absent means today's behaviour,
      byte-for-byte — proven by the committed documents/pack/web not moving.
- [x] The canonical document (`document.rs::DocOAuth2`), the manifest serialization
      (`auth.rs::OAuth2Spec`), and `catalog::OAuth2` all carry the field (additive; document schema
      gains the optional property, SCHEMA_VERSION stays 1 per the forward-compat contract — an
      older reader tolerates it, `catalog-reader::additive_growth_is_tolerated`).
- [x] Failing-first loader test:
      `oauth_token_endpoint.rs::a_two_host_declaration_loads_and_carries_both_services` and
      `::a_dangling_token_endpoint_is_refused_naming_it`.
- [x] The consumer contract is recorded in the field's doc (`auth.rs`, `catalog/lib.rs`) and the
      design doc (`docs/designs/catalog-artifact.md`): `token_path` resolves against
      `token_endpoint`'s service base URL when set, `endpoint`'s otherwise (X-154 declared-defaults).
- [x] **Extension 2 (folded in per coordinator):** `OAuth2Spec.public_client: bool` discriminator;
      `auth_archetypes.rs::every_oauth_connector_generates_the_operator_connection_split` now
      requires the secret `oauth.client_secret` field only of a confidential client, proven by
      `::a_public_client_is_exempt_from_the_client_secret_a_confidential_one_owes`.
- [~] Full gate green; `diff` clean **except** the additive schema property and the fenced
      `connectors.lock` (stale because it hashes the 3 Rust-catalog tables that regenerate — a
      breaking `catalog::OAuth2` change the coordinator accepted; coordinator regenerates the lock
      at integration). No document/manifest/pack/web artifact moves.

## Progress

- 2026-08-12: Filed from C-555's measured model gap, after the operator decided to ship both
  Anthropic OAuth2 flows. The subscription flow (claude.ai authorize + platform.claude.com token)
  is the first two-host consumer; the console flow is single-host and needs nothing from here.
- 2026-08-12: Implemented both extensions on `impl/C-556`. `OAuth2Spec` gains `token_endpoint`
  (second-service reference, loader-validated) and `public_client` (PKCE discriminator); both are
  additive-optional and skipped when absent/false, so no document, manifest, pack, or web artifact
  moves. Carried into `DocOAuth2` + document schema (the one expected artifact change) and
  `catalog::OAuth2`. The Rust-catalog change is a breaking, non-`#[non_exhaustive]` public-API
  change: the 3 tables that render an `Acquisition::OAuth2` literal (babelforce, github, gitlab)
  regenerate, which leaves the fenced `connectors.lock` stale (3 hashes) for the coordinator to
  regenerate. The other 17 `[auth.oauth2]` providers render no OAuth2 literal in the Rust catalog
  today (their catalogue credential is `Static`), so their tables are unaffected. Full gate run;
  the only red is `lockfile::the_committed_lockfile_is_a_fixed_point_of_a_build`, the expected
  whole-catalogue staleness.

- 2026-08-12: Implemented on `impl/C-556` (`44f34a81`), merged and lock-regenerated at
  integration. Two additive OAuth2Spec fields — `token_endpoint` (a second declared service the
  token_path resolves against, a name never a URL) and `public_client` (a PKCE client that owes no
  client secret). The C-22 archetype gate now requires the operator client_secret only of a
  CONFIDENTIAL authorization_code client; the exemption is fail-safe — `public_client` defaults
  absent/false, so a connector must explicitly opt into public and nothing escapes by omission
  (proven both directions over fixtures). Both fields skip when absent, so every existing document
  and manifest is byte-identical; `catalog::OAuth2` gains both (a deliberate breaking addition to
  the published crate, its two exhaustive downstream sites updated) and the document schema gains
  two optional properties. C-555 round 2 is the first declarer.

## Notes

- Write set: `crates/connector-spec/src/auth.rs`, the document lowering in
  `crates/connector-cli` (document.rs/catalog.rs/