---
id: C-75
title: Ship the Airtable connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · `fields` envelope · base and table in the path
---

# Ship the Airtable connector

## Goal
Ship the spreadsheet-database everyone actually stores their operational data in: record
read, create and update, addressed by base, table and record id.

## Acceptance
- [ ] `providers/airtable.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://api.airtable.com"`, `vendor = "Airtable"`, and a `[[auth]]` entry with `scheme = "bearer"` over `AIRTABLE_ACCESS_TOKEN`, named by `default_auth`.
- [ ] A curated set of roughly four over `/v0/{baseId}/{tableIdOrName}`: record get, record
      create, record update, record delete.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/airtable.flux` and
      `connectors/airtable.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **The `fields` envelope is declared through `wire` paths** — Airtable wraps a record's values in
      `{{"fields": {{…}}}}`, and the flat form is rejected.
- [ ] `{baseId}` and `{tableIdOrName}` are **path parameters, and the story states why that is safe**:
      path values are as unencoded as query values today, and Airtable ids are restricted to
      `[A-Za-z0-9]`. A table *name* in the path is not safe by that argument — either require the id
      form or say what happens with a name containing a space.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- The table-name case is the first place in this repo where the unencoded-path-parameter question
  has a real answer rather than a lucky one (`openai-model-get` was safe by the vendor's id charset).
- Deliberately excluded pending C-30: `listRecords` with its `filterByFormula`, which is both a query
  value and a formula language.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
