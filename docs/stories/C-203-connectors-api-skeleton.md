---
id: C-203
title: "The connectors-api service, the tenancy model, and one live call"
pillar: Bridge
status: ready
priority: 2
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, host]
note: "the vertical slice: paste a token, pick an operation, get a real vendor response. No sign-in, no OAuth, no UI beyond what proves it"
---

# The `connectors-api` service, the tenancy model, and one live call

## Goal

A running HTTP service that binds the three ports per tenant, installs a pack, and executes an
operation against a real vendor — the first thing in this repository that calls anyone.

## Acceptance

- [ ] `crates/connectors-api` exists with `publish = false`, and is a leaf: nothing in the workspace
      depends on it, asserted by [C-202](C-202-flux-web-egress.md)'s fence extension.
- [ ] The tenancy model is explicit and is **a parameter of every port**, not a global: a request
      resolves to a tenant, and `Credentials::new(store, tenant)` / `Configuration::new(values,
      tenant)` are constructed for that tenant. `Error::TenantMismatch` is unreachable by
      construction, and a test proves the pairing cannot be crossed.
- [ ] **Failing-first:** an integration test that stores a credential for tenant A, executes an
      operation as tenant A against a loopback vendor, and asserts the request carried A's credential.
      A second test does the same as tenant B and asserts B's credential — never A's.
- [ ] `POST /v1/operations/{id}/execute` runs a catalogue operation for the session's tenant and
      returns the vendor response. It calls `connector_pack::pack`; it constructs no request.
- [ ] `GET /v1/connectors` and `GET /v1/operations` serve the catalogue — from
      `catalog::providers()`, not from a hand-maintained list.
- [ ] A credential can be written for a tenant (`PUT /v1/tenants/{tenant}/credentials/{name}`) and the
      response **never echoes it back**, including on error. Asserted, not assumed.
- [ ] Manual, documented, and labelled manual: one real call to one real vendor with a real token,
      with the response and the redactor's view of it both shown.

## Notes

- Follow [connectors-app.md](../designs/connectors-app.md) §"Vertical slice 1" for the sequence; only
  the tenancy and the binding differ under [C-201](C-201-charter-multi-tenant-host.md).
- Nine connectors need a `Configuration` value before their URL resolves — `zendesk`, `shopify`,
  `jira`, `freshdesk`, `salesforce`, `docusign`, `okta`, `contentful`, `statuspage`, covering 53 of
  248 operations. Pick a first vendor **without** a templated base URL (`anthropic`, `slack`,
  `vercel`, `datadog`, `fly`, `postmark`) so the slice does not need the config surface too.
- The `ConfigStore` stability requirement is a real constraint on a service that reads a database:
  *"an implementation must answer the same `(tenant, provider, field)` with the same value for as long
  as the store is bound"*. Resolve eagerly per request and hand over a fixed set.
