---
id: C-72
title: Ship the HubSpot connector
pillar: Spec
status: done
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
- [x] `providers/hubspot.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [x] `base_url = "https://api.hubapi.com"`, `vendor = "HubSpot"`, and a `[[auth]]` entry with `scheme = "bearer"` over `HUBSPOT_ACCESS_TOKEN`, named by `default_auth`.
- [x] A curated set of roughly five over `/crm/v3/objects`: contact get, contact create,
      contact update, company get, deal get — each addressed by object id in the path.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [x] `cargo run -p connector-cli -- build` emits `connectors/hubspot.flux` and
      `connectors/hubspot.connector.toml`, both committed, and a second build is byte-identical.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [x] **The `properties` envelope is declared through `wire` paths.** HubSpot wraps writes in
      `{{"properties": {{…}}}}`; the flat form is silently accepted and stores nothing.
- [x] A private-app token is the only credential; the legacy `hapikey` query parameter is **not
      modelled** — it is deprecated, and it would put a live secret in a query string, which is both a
      credential leak and unencodable (C-30). Record the exclusion the way babelforce records its
      deprecated header pair.

## Progress
- **Done.** `providers/hubspot.toml` ships 5 operations; `connectors/hubspot.flux`,
  `connectors/hubspot.connector.toml`, `crates/catalog/{ops/hubspot/*.flux,src/generated/hubspot.rs}`
  and `web/public/catalog.json` are regenerated and committed. `diff` reports
  `67 artifacts up to date (7 providers checked)`.
- Contract asserted in `crates/connector-flux/tests/hubspot_connector.rs` (10 tests): the bearer, the
  empty query surface on the IR *and* on the emitted text, the `properties` envelope as the exact
  emitted payload record, no optional body field, the write metadata, the single host, and that
  nothing named `hapikey` reaches the IR in any request position. The curated count is
  `("hubspot", 5)` in `connector-spec`'s `operation_selection_stays_curated`.
- Every operation reports exactly one issue in the public catalogue — `credential-not-injected`, the
  catalogue-wide C-10 gap. No `unencodable-query-value` and no `unbound-base-url-template`, which is
  the cleanest a shipped provider can currently be.
- **Left out for C-56** (an omitted optional body field travels as an explicit `null`): on
  `hubspot-contact-create`, `firstname`, `lastname`, `phone` and `company` — so a create takes only
  the email, and the name needs a second call to `hubspot-contact-update`. On
  `hubspot-contact-update`, every property other than `firstname`/`lastname`, and with them the whole
  idea of a *partial* update: both declared fields are written on every call, which the operation's
  description states because it is the caller-visible cost. `email` is deliberately not writable on
  the update — it is HubSpot's unique identifier, so changing it can 409 or provoke a merge.
- **Correction to this story's Notes, recorded rather than absorbed:** the search endpoints are *not*
  blocked by C-30. `POST /crm/v3/objects/contacts/search` takes a JSON body, the shape
  `providers/slack.toml` relies on to avoid the encoding gap entirely. What blocks it is that its
  filter DSL is an array of objects and `Param::wire` addresses nested **records** only (`body_tree`
  builds a `BTreeMap`, so a `0` segment emits the literal key `"0"`). A free-form `body_schema` would
  compile but would publish HubSpot's operator vocabulary to a model as an untyped `Any`, and a search
  that matches nothing looks identical to one with no results. Filed in the provider file as a schema
  gap; closing it needs array support in `wire`.
- **Second schema gap, same file:** a vendor-*constant* query parameter has nowhere to live, so
  "get the contact with this email address" (`?idProperty=email`) is unexpressible even once C-30
  lands. Identical in class to the constant `Accept` header `providers/github.toml` records — now
  observed in a second request position on a second provider, which is the argument for a
  constant-value field the emitter binds as a literal.
- `quirks.rate_limit` is deliberately **not** declared: HubSpot's limits vary by subscription tier and
  `RateLimit` takes an exact `requests`/`per_seconds` pair that is published verbatim to consumers, so
  a bound recalled rather than read would be a wrong contract a model tries to satisfy.

## Notes
- The `hapikey` exclusion is the same class of decision as babelforce's deprecated
  `X-Auth-Access-*` pair: known and excluded must be distinguishable from absent (C-19).
- Deliberately excluded pending C-30: every `properties=` projection, `propertiesWithHistory`,
  `associations`, `archived` and `idProperty`. The projection is the one that costs something: without
  it every read returns only the object type's default property set, so no custom property is
  reachable. The search endpoints are excluded too, but for a different reason — see Progress.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
