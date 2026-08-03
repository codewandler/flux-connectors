---
id: C-86
title: "The connector configuration surface — enough declared data to generate the UI (epic)"
pillar: Spec
status: backlog
design: docs/designs/connector-configuration.md
epic: connector-config
areas: [connector-spec, connector-cli, providers, bridge]
note: "EPIC — the two-level configuration vocabulary landed; catalogue projection and hosted OAuth remain explicit backlog stories"
---

# The connector configuration surface — enough declared data to generate the UI (epic)

## Goal
Let a hosted product render a working "Connect this integration" form from a connector alone — the
fields, their labels, their validation, where each answer goes, and what to do about webhooks — without
anyone reading the provider TOML.

## Acceptance
- [x] **The two-level model is recorded and enforced.** Operator level (the OAuth app registration) is
      distinct from connection level (the tenant's own values), and `Level` is **derived** from what a
      field binds rather than authored, so an author cannot get it wrong.
- [x] A `[[config]]` field carries what a form needs — label, help, example, format, required, secret,
      docs URL — and `binds` says where the collected value goes, keyed to names flux already owns.
- [x] **Every rule is a refusal.** A connector asks for everything it needs and nothing it cannot use;
      `secret` must agree with `binds`; a field must be renderable; an example must satisfy its own
      format.
- [x] **The recorded gap closes.** `{subdomain}`, `{site}`, `{shop}` and `{domain}` are declared, their
      `SCHEMA GAP:` comments deleted, and a test asserts no shipped provider leaves a template
      variable unbound — [C-68](C-68-endpoint-binding.md)'s central acceptance.
- [x] A `verify` operation is declarable, so a "Test connection" button has something to call. The
      convention already existed invisibly in three providers.
- [x] Webhooks are a full exposure: `[channels.subscription]`, `[channels.setup]`, and per-event
      `default`/`group`. A `webhook` binding must declare one of the first two.
- [x] The auth archetypes are pinned as *forms* — [C-22](C-22-auth-conformance-matrix.md)'s matrix,
      asked from the configuration side, with OAuth as an explicit failing case.
- [ ] Config, `verify` and the setup blocks reach the manifest and `catalog.json` —
      [C-87](C-87-configuration-codegen.md).
- [ ] One provider proves OAuth end to end — [C-88](C-88-prove-oauth2.md).
- [ ] The hosted redirect gap is closed — [C-89](C-89-hosted-oauth-redirect.md).

## Progress
- 2026-08-03 — Returned the umbrella to backlog after the configuration IR and refusal rules
  landed. C-87–C-89 retain the remaining catalogue and hosted-OAuth work without presenting the
  umbrella itself as an actively staffed lane.
- 2026-07-30 — **IR and loader landed.** `crates/connector-spec/src/config.rs` carries `ConfigField`,
  `Format`, the `binds` grammar and `Level`; `Connector` gained `config` and `verify`, both inside the
  hash domain. 22 tests in `tests/config_fields.rs`, 9 in `tests/auth_archetypes.rs`.
- 2026-07-30 — All four templated providers declare their tenant field and lost the `SCHEMA GAP:`
  comment they had carried since C-17. Zendesk is the fullest form the fleet has: subdomain, agent
  email, API token — three fields across three binding forms.
- 2026-07-30 — Fixed `first_template_variable`, whose name described its bug: a base URL with two
  variables reported one, and the other was invisible to every consumer of `catalog.json`'s status.
- 2026-07-30 — Two things the design did not anticipate: `example` is validated against `format` (a
  placeholder that fails its own field is worse than none, because a user copies it), and `verify`
  refuses a high-risk operation (a connection test runs unattended whenever someone opens a settings
  page).

## Notes
- **The `description` field could not be reused.** It is the text a model receives as a tool contract
  (`site.rs` documents it as such), and `providers/slack.toml` showed the strain — one sentence
  carrying a label, a placeholder and a scope list.
- **Do not duplicate flux.** `EndpointSpec::template` already composes a URL from `{placeholder}`
  values host-side; `ConfigSpec` vs `AuthMethod` is a type-level secret partition the host enforces.
  We name destinations; flux resolves them.
