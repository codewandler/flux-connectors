---
id: C-74
title: Ship the Shopify connector
pillar: Spec
status: done
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
- [x] `providers/shopify.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
      → `providers/shopify.toml:1-19`
- [x] `base_url = "https://{shop}.myshopify.com"`, `vendor = "Shopify"`, and a `[[auth]]` entry with `scheme = "header"` on `X-Shopify-Access-Token` over `SHOPIFY_ACCESS_TOKEN`, named by `default_auth`.
      → `providers/shopify.toml:153,136,166-174`; the TOML spelling of the externally-tagged variant is
      `scheme = { header = { name = "X-Shopify-Access-Token" } }`. Asserted by
      `shopify_connector.rs::the_shopify_connector_authenticates_with_a_plain_custom_header`.
- [x] A curated set of roughly five over `/admin/api/<version>`: order get, product get, product
      update, customer get, inventory level get — each addressed by id in the path.
      → five ops in `providers/shopify.toml`; count pinned in
      `connector-spec/tests/shipped_providers.rs::operation_selection_stays_curated`. **"inventory
      level get" ships as `shopify-inventory-level-list`** over
      `/locations/{location_id}/inventory_levels.json` — see Progress for why the endpoint the story
      names cannot be expressed.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
      → `shopify_connector.rs::no_shopify_operation_declares_a_query_parameter` (IR) and
      `::no_shopify_operation_emits_a_query_string` (every `$url` line, no `?`, no `$sep`).
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
      → `shopify_connector.rs::no_shopify_operation_declares_an_optional_body_field`; exclusions in
      Progress.
- [x] `cargo run -p connector-cli -- build` emits `connectors/shopify.flux` and
      `connectors/shopify.connector.toml`, both committed, and a second build is byte-identical.
      → `cargo run -p connector-cli -- diff` reports `67 artifacts up to date (7 providers checked)`.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
      → `shopify_connector.rs::every_shopify_operation_emits_a_module_that_parses_analyzes_and_is_canonical`,
      plus every derived gate (C-54): `shipped_modules.rs`, `shipped_providers_build.rs`,
      `catalog_artifacts.rs`, `site_catalog.rs`, `embedded_operations.rs`.
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
      → `shopify_connector.rs::no_credential_and_no_widened_host_reaches_a_generated_module`: the module
      names exactly one absolute URL and it is `$base`; it carries neither `SHOPIFY_ACCESS_TOKEN` nor
      `X-Shopify-Access-Token`. `hosts` in `web/public/catalog.json` is `{shop}.myshopify.com`.
      Repository-wide: `shipped_providers.rs::no_provider_file_carries_a_credential_value` and
      `site_catalog.rs::no_credential_value_reaches_the_document`.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
      → `shopify-product-update` is `risk = "high"` / `idempotency = "non_idempotent"`, reasoned at
      `providers/shopify.toml:235-253` and asserted by
      `shopify_connector.rs::the_product_update_is_the_only_write_and_declares_itself_as_one`.
- [x] **The API version in the path is called out as the service-versioning case it is.** Shopify
      spells its version into every URL (`/admin/api/2024-10/…`), so the connector's `api_version` and
      its path must agree by construction rather than by a hand-edited string in each operation. Say
      how C-49's per-service `api_version` supplies it.
      → `providers/shopify.toml:38-79` states the gap and the intended C-49 relationship;
      `shopify_connector.rs::the_api_version_is_one_value_across_every_path` holds one constant against
      all five paths and all five emitted URLs. **The field does not exist yet, so the version is still
      typed five times** — the test is what keeps them in agreement in the meantime.
- [x] The `{shop}` tenant template is recorded as an unbound base URL (C-68), as zendesk's is.
      → `providers/shopify.toml:138-152` (SCHEMA GAP, no binding invented);
      `shopify_connector.rs::the_shop_tenant_template_reaches_the_module_unbound`; `status.rs` derives
      `unbound-base-url-template` for all five operations in `web/public/catalog.json`.
- [x] The credential is the whole header value — no prefix — which is why this fits
      `AuthScheme::Header` exactly and needs nothing from C-19's prefix axis.
      → `providers/shopify.toml:21-37`; the scheme round-trips to
      `{"kind":"header","name":"X-Shopify-Access-Token"}` in `web/public/catalog.json`.

## Progress
- **Done (C-74).** Five operations, first shipped use of `AuthScheme::Header`, gate green.
- **Left out for C-56 (optional body fields emit an explicit `null`).** `shopify-product-update`
  declares `product.title` and nothing else. Excluded: `body_html`, `status`, `vendor`,
  `product_type`, `tags`, `handle`, `published_at`, `template_suffix` and the whole `variants` /
  `options` / `images` array surface. Shopify's product update is a *partial* update where an absent
  key means "leave this alone" and a `null` means "clear it", so an optional `body_html` would let a
  caller who set only `title` wipe the description and get a 200 back. `POST
  /inventory_levels/set.json` was considered *instead* — its three body fields are all required, so
  C-56 does not bite it — and rejected as out of the story's set: it is a stock mutation, not a read,
  and it takes no id in the path.
- **The API version, honestly.** `/admin/api/2024-10/` is typed into all five paths, because no schema
  field carries a version and `base_url` is the host `http_hosts` derives from. The intended shape is
  C-49's per-service `api_version`: the path becomes the version-free remainder
  (`/orders/{order_id}.json`) and the emitter composes `{base}/admin/api/{api_version}` in front of it,
  making the agreement a property of the emitter rather than of an author's diligence. Until then
  `the_api_version_is_one_value_across_every_path` fails a partial bump; when C-49 lands it becomes the
  check that the derivation happened.
- **"inventory level get" became a per-location list, deliberately.** Shopify identifies an inventory
  level by an (`inventory_item_id`, `location_id`) *pair*, so there is no `.../inventory_levels/{id}`
  endpoint. `GET /inventory_levels.json` — the one the story means — **requires**
  `inventory_item_ids` or `location_ids` as query parameters, so it is unexpressible while C-30 is
  open. `GET /locations/{location_id}/inventory_levels.json` is the same data behind a path id and no
  query parameter, so that is what ships, first page only (no cursor pagination — see Notes).
- **Two schema gaps recorded rather than worked around**, both in the provider header comment:
  Shopify's leaky-bucket rate limit (capacity 40, refill 2/s) is not a fixed `requests`/`per_seconds`
  window, so no `rate_limit` is declared; and its `errors` field is polymorphic (a string on 404, an
  object of field→messages on 422) while `ErrorEnvelope.message_pointer` is a single pointer, so
  `/errors` is declared for the location and says nothing about the shape.
- **Adjacent, not fixed:** `Risk` grades what a call *changes*, so `shopify-customer-get` — a read of a
  named individual's contact details and purchase history — is `low` on the same scale as reading a
  product title. There is no confidentiality axis for a read that discloses personal data. The fact is
  stated in the operation's `description`, which is what reaches the model, and the gap is named at
  `providers/shopify.toml:295-301`.

## Notes
- Shopify is the only vendor in this fleet whose auth needs no `Authorization` header at all, which
  makes it the cheapest proof that the `Header{{name}}` variant round-trips end to end.
- Deliberately excluded pending C-30, named endpoint by endpoint (reasoned in
  `providers/shopify.toml:82-110`): `GET /orders.json` (`status`, `financial_status`,
  `fulfillment_status`, `created_at_min` — enum and timestamp strings); `GET /products.json` (`title`,
  `vendor`, `handle`, `collection_id`, where merchant text routinely carries `&`);
  `GET /customers/search.json?query=…`, the direct analogue of `zendesk-ticket-search`;
  `GET /inventory_levels.json`, which *requires* `inventory_item_ids` or `location_ids`; and the
  `?limit=` / `?page_info=` cursor paging every collection endpoint uses, which
  `Pagination::Cursor.cursor_param` can only spell as a query parameter. GraphQL is excluded too: it is
  a single `POST /admin/api/<v>/graphql.json` whose whole contract is a free-form query document, which
  would be one `Any`-typed body parameter and no operation contract at all.
- The one consequence a consumer sees: `shopify-inventory-level-list` returns the first page only. That
  is a known incompleteness, stated in the operation's own description, rather than a silent truncation
  the quirk model pretends to handle.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
