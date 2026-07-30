---
id: C-101
title: Make services a visible, filterable dimension
pillar: Surfaces
status: done
priority: 4
design: docs/designs/explorer-ux.md
epic: explorer-ux
areas: [web]
note: "18 services are published with base_url, api_version, gid and operation counts — the explorer mentions none of them, so Google's three read as one"
---

# Make services a visible, filterable dimension

## Goal
Surface the middle addressing level. [C-49](C-49-provider-services.md) made a service the unit you
address, version, select and install; the catalogue publishes 18 of them and the explorer shows none.

## Acceptance
- [x] `ProviderCard.vue` shows a provider's services when it has more than the reserved `default` one
      — name, operation count, and the `api_version` where it differs from the provider's. A
      single-surface provider grows no services list, so fifteen cards do not gain a row that says
      nothing. **Amended at close:** `slack` does gain one row — an `Address` fact carrying its
      published `com.slack.api:v1`. That gid is the reserved service's with `default` already
      elided by the address grammar, so the reserved name is still rendered nowhere; the strict
      alternative would have satisfied the `gid` item below with zero instances, which is the
      vacuous pass this story's own test discipline exists to prevent.
- [x] The operation list gains a **service filter**, populated from the catalogue like every other
      facet — add a service and the filter grows with no edit to the component.
- [x] The service filter is dependent in the obvious way: choosing a connector narrows the service
      options to that connector's. Choosing a service with no connector selected is still valid.
- [x] An operation row states which service it belongs to when the provider has more than one.
- [x] The `gid` is shown where a service has one, since it is the address a consumer copies. Today only
      `slack` has one, so the UI must read correctly when it is `null` — which is fifteen of sixteen.
- [x] Failing-first test in `web/test/explorer.test.mjs` for the new selector in `web/data/catalog.mts`
      — the facet and the dependent narrowing are pure functions and belong there.

## Progress
- Done on `impl/C-101`. `web/data/catalog.mts` gains the `Service` type, `services`/`authority`/
  `api_version` on `Provider`, `service` on `Operation`, and five pure selectors: `namedServices`,
  `serviceFacet`, `operationService`, `serviceApiVersion`, `providerAddress`. The reserved name lives
  in exactly one private constant there, so nothing else in the site knows it.
- `ProviderCard.vue` lists the named services with count and differing version; `OperationList.vue`
  gains the dependent service select; `OperationRow.vue` labels a row when its connector addresses
  more than one surface. No layout file was touched — C-100 owns those.
- `web/test/explorer.test.mjs` gains three tests, all failing at the merge base: the facet and its
  narrowing, the card and row rendering, and the address/null case. The hand-maintained-data guard
  now also forbids service names and published addresses in the explorer sources, with the reserved
  name as the one documented exception.
- **One editorial call to review:** the only service carrying a `gid` today is a reserved one, so
  rendering the address strictly per service would have satisfied that acceptance vacuously. The card
  therefore shows an **Address** fact for a single-surface connector that publishes one — which
  changes exactly one of the fifteen single-surface cards, and never renders the reserved name. See
  the implementor's report.

## Notes
- `catalog.json` already carries everything needed: `ServiceEntry { name, description, base_url,
  hosts, api_version, gid, operation_count }`, and every operation carries its `service`.
- The `default` service is **never rendered** — it is elided from every address by design
  (`docs/designs/provider-services.md`), and showing it in a UI would contradict that.
