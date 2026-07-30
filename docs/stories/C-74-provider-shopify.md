---
id: C-74
title: Ship the Shopify connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: custom header credential · API version lives in the path
---

# Ship the Shopify connector

## Goal
Ship commerce, and with it the first connector whose credential is a plain custom header
rather than an `Authorization` scheme — which is the `Header{{name}}` variant flux already understands.

## Acceptance
- [ ] `providers/shopify.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://{shop}.myshopify.com"`, `vendor = "Shopify"`, and a `[[auth]]` entry with `scheme = "header"` on `X-Shopify-Access-Token` over `SHOPIFY_ACCESS_TOKEN`, named by `default_auth`.
- [ ] A curated set of roughly five over `/admin/api/<version>`: order get, product get, product
      update, customer get, inventory level get — each addressed by id in the path.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/shopify.flux` and
      `connectors/shopify.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **The API version in the path is called out as the service-versioning case it is.** Shopify
      spells its version into every URL (`/admin/api/2024-10/…`), so the connector's `api_version` and
      its path must agree by construction rather than by a hand-edited string in each operation. Say
      how C-49's per-service `api_version` supplies it.
- [ ] The `{shop}` tenant template is recorded as an unbound base URL (C-68), as zendesk's is.
- [ ] The credential is the whole header value — no prefix — which is why this fits
      `AuthScheme::Header` exactly and needs nothing from C-19's prefix axis.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- Shopify is the only vendor in this fleet whose auth needs no `Authorization` header at all, which
  makes it the cheapest proof that the `Header{{name}}` variant round-trips end to end.
- Deliberately excluded pending C-30: every collection endpoint (`?status=`, `?limit=`) and GraphQL.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
