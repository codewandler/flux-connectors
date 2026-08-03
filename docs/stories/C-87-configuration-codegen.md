---
id: C-87
title: Publish the configuration surface into the manifest and the catalogue
pillar: Codegen
status: done
design: docs/designs/connector-configuration.md
epic: connector-config
areas: [connector-cli, catalog, web]
note: "M1 critical: complete config/verify projection is required for UI and CLI onboarding; also includes a BREAKING change because site.rs flattens OAuth2Spec to `oauth2: bool`"
---

# Publish the configuration surface into the manifest and the catalogue

## Goal
Make the configuration surface reachable by a consumer. It is in the IR and in the hash domain and
reaches no artifact, so a product still cannot render a form without parsing provider TOML.

## Acceptance
- [x] `connectors/<id>.connector.toml` carries a `[[config]]` block per field — name, label, help,
      example, format, required, secret, docs URL, `binds`, and the **derived** level — plus `verify`
      and each binding's `subscription` / `setup`.
- [x] `catalog.json` carries the same. Additive for those keys, so no `SCHEMA_VERSION` bump on their
      account.
- [x] **The OAuth flattening is fixed, and it is breaking.** `crates/connector-cli/src/site.rs`
      collapses the entire `OAuth2Spec` to `oauth2: bool`, discarding `scopes`, `grants`,
      `authorize_path`, `token_path`, `client_id` and `redirect` — so a hosted product cannot build an
      authorize URL from the catalogue at all. Publishing the spec changes an existing field's type,
      which the [catalog-json](../designs/catalog-json.md) contract says bumps `SCHEMA_VERSION` 2 → 3.
      Decide and record whether to bump or to add a second key; bumping is the honest option, since the
      boolean was lossy by accident rather than by design.
- [x] **No credential value reaches any artifact.**
      `crates/connector-cli/tests/site_catalog.rs::no_credential_value_reaches_the_document` runs with
      a sentinel for every config-bound env var too.
- [x] `--service <name>` selects that service's config fields along with its operations.
- [x] **Nothing reaches the `.flux` module.** A test asserts every shipped module is byte-identical
      across this story: configuration describes what a human supplies, and reaches no generated code.
- [x] The public site renders a provider's configuration surface — or the story records why not. This
      is the first artifact that would let the site show more than `auth: bearer`.

## Progress
- 2026-08-04: Done. Manifests, the schema-v3 public catalogue, the embedded declaration JSON and
  the public explorer now publish complete configuration and verification surfaces; generated Flux
  remains unchanged by configuration projection.
- 2026-08-03: Raised to Milestone 1 priority. Until the complete config and `verify` projection
  reaches consumer artifacts, neither the Exchange console nor the Flux CLI can honestly collect
  connector settings such as a Zendesk domain or C-508's self-managed GitLab origin.

## Notes
- `catalog.rs` and `site.rs` share the credential and host walks deliberately, so a site and a
  `cargo add` consumer cannot be told different things about the same connector. Keep that.
- The manifest is still the least informative of the four backends — it carries no credentials at all
  (C-10). This story should not outrun that: publishing config that references credentials the
  manifest does not name would be half a picture.
