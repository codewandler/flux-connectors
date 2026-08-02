---
id: C-458
title: "Preserve a published default service while a connector grows"
pillar: Spec
status: done
priority: 1
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [connector-spec, connector-flux, connector-cli]
note: "prerequisite — Zendesk Support already publishes the elided default address; named Help Center or Messaging must not repoint it"
---

# Preserve a published default service while a connector grows

## Goal
Let a connector explicitly preserve a legacy `default` service beside named services, while keeping
omission ambiguous and therefore refused.

## Acceptance
- [x] A failing-first fixture proves the current loader refuses a default service beside a named one.
- [x] The accepted representation distinguishes an explicit legacy default from an omitted service;
      every member in a mixed connector must name its owner.
- [x] The legacy default keeps its elided GID/OIP, credential address, and unsuffixed connector/module
      paths; a named sibling gets its service segment and suffixed artifacts.
- [x] Roles, tags, configuration, verification, graphs, events, and channels remain attached to the
      service that owns them.
- [x] Default-only and named-only providers remain byte-identical and the address round-trip/property
      tests cover the mixed shape.
- [x] The design records why this is an address-migration capability, not a new-provider shorthand.

## Progress
- 2026-08-02: failing-first `cargo test -p codewandler-connector-spec --test
  legacy_default_service an_explicit_legacy_default_can_coexist_with_a_named_service -- --exact`
  exited 101 because `Service` had no `legacy` field; the pinned
  `default-service-beside-a-named-one` rejection records the old loader refusal.
- 2026-08-02: `[[services]] name = "default" legacy = true` now admits only the address-migration
  shape beside at least one named sibling. The loader retains raw `service`-key presence through
  validation and refuses omission on operations, spec documents, events, channel bindings,
  configuration fields, and graphs in that shape.
- 2026-08-02: focused specification tests, all ten `service_units` tests, and `cargo check
  --workspace` pass. The committed-catalogue fixed-point test within `service_units` proves existing
  default-only and named-only artifacts remain byte-identical.

## Notes
- Zendesk is the regression fixture: its current Support OIPs remain in `com.zendesk.api:v2#…`.
