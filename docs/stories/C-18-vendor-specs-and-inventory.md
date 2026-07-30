---
id: C-18
title: Vendor provider specs and curate the operation inventory
pillar: Spec
status: ready
priority: 4
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [providers]
note: prerequisite half of C-17 · pure research, no Rust
---

# Vendor provider specs and curate the operation inventory

## Goal
Gather and commit the raw material the three provider configs will be written from: a vendored,
hermetic spec cache plus a curated inventory of exactly which operations each connector should expose
and how each authenticates. Doing this before the TOML schema exists means C-17 becomes transcription
rather than research.

## Acceptance
- [ ] `specs/babelforce/` holds the vendored babelforce OpenAPI document, committed, with its upstream
      path and version recorded.
- [ ] `docs/designs/provider-operation-inventory.md` lists, per provider, the selected operations with
      method, path, parameters (name / in / type / required), and a one-line description.
- [ ] Selection is **curated and justified** — babelforce's spec carries 163 operations and must
      yield a usable handful. Each inclusion earns its place; the doc says why the rest are out.
- [ ] Auth is recorded per provider using the `flux_plugin_protocol::AuthScheme` vocabulary
      (`bearer` / `basic` / `header{name}` / `query{name}`), including the requirement-set shape
      (AND / OR / none) per operation.
- [ ] The deprecated babelforce `X-Auth-Access-Id` / `X-Auth-Access-Token` pair is recorded as
      **excluded**, with the reason, so a later reader does not "fix" its absence.
- [ ] No Rust, no `providers/*.toml` — the TOML schema does not exist yet (C-3).

## Progress
- (not started)

## Notes
- Sources:
  - **zendesk** — `../flux/plugins/zendesk/src/main.rs` (687 lines) is authoritative for the operation
    set and the `<email>/token` Basic form.
  - **freshdesk** — `~/babelforce/projects/integrations/action-proxy/dist/collections/freshdesk/freshdesk.yml`
    (649 lines): endpoints, params, and Basic auth with `api_key` as user and literal `X` as password.
  - **babelforce** — `~/babelforce/projects/babelforce-api/babelforce-api/openapi/manager.openapi.json`
    (OpenAPI 3.0.3, 98 paths, 163 operations; servers include `https://services.babelforce.com`).
- Babelforce auth is **SSO-issued Bearer**, with **JWT planned**. Record the JWT intent so C-10's
  manifest schema is designed to accept it.
- action-proxy is the cautionary tale, not a template: it hand-maintained 649 lines of YAML per
  provider and needed a bespoke `{{base64:encode …}}` template function for Zendesk auth. Mine it for
  endpoint facts only.
