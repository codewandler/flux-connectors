---
id: C-72
title: Ship the HubSpot connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer private-app token · `properties` envelope
---

# Ship the HubSpot connector

## Goal
Ship the CRM half of the fleet: contacts, companies and deals as typed operations, so an
agent can read and update a customer record without a bespoke integration.

## Acceptance
- [ ] `providers/hubspot.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://api.hubapi.com"`, `vendor = "HubSpot"`, and a `[[auth]]` entry with `scheme = "bearer"` over `HUBSPOT_ACCESS_TOKEN`, named by `default_auth`.
- [ ] A curated set of roughly five over `/crm/v3/objects`: contact get, contact create,
      contact update, company get, deal get — each addressed by object id in the path.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/hubspot.flux` and
      `connectors/hubspot.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **The `properties` envelope is declared through `wire` paths.** HubSpot wraps writes in
      `{{"properties": {{…}}}}`; the flat form is silently accepted and stores nothing.
- [ ] A private-app token is the only credential; the legacy `hapikey` query parameter is **not
      modelled** — it is deprecated, and it would put a live secret in a query string, which is both a
      credential leak and unencodable (C-30). Record the exclusion the way babelforce records its
      deprecated header pair.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- The `hapikey` exclusion is the same class of decision as babelforce's deprecated
  `X-Auth-Access-*` pair: known and excluded must be distinguishable from absent (C-19).
- Deliberately excluded pending C-30: the search endpoints and every `properties=` projection.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
