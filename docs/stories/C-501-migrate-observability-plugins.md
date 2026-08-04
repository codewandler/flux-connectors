---
id: C-501
title: "Migrate the observability plugins into connectors"
pillar: Connector
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [providers, runtime, observability]
note: "Alertmanager, Grafana, Loki, Opsgenie and Prometheus become catalogue-owned integrations, including streaming/tailing and datasource contracts"
---

# Migrate the observability plugins into connectors

## Goal

Move Alertmanager, Grafana, Loki, Opsgenie and Prometheus from native Flux plugins to connector
definitions and, only where generation is insufficient, connector-owned runtime artifacts.

## Acceptance

- [ ] Every existing operation, datasource contribution, schema, effect and refusal is mapped to a
      connector member or an explicit reviewed removal.
- [ ] **No plugin whose manifest declares datasources is deleted until every declaration is mapped**
      — Decision 0006 rule 11, as a checkable gate rather than prose: this story carries a
      per-plugin mapping table listing each declared datasource by name, and each row resolves to a
      published `[[datasources]]` connector member proven through C-505's conformance harness or to
      an explicit reviewed removal recorded here. A wave with an unresolved row does not dispatch.
- [ ] Plain HTTP surfaces are generated; protocol-specific streaming or query behavior uses a
      declared runtime adapter rather than vendor code in Flux.
- [ ] Loki tail/follow and equivalent long-running reads have bounded stream and cancellation
      semantics suitable for the Exchange connector WebSocket.
- [ ] C-505's legacy-plugin-versus-Exchange conformance covers at least one read, one mutation where
      supported, one datasource projection and one long-running operation.
- [ ] Flux receives a verified five-crate retirement list with replacement addresses.

## Progress

- (not started)

## Notes

- This is migration wave 3 after C-500.
- **Amended 2026-08-04 by C-510:** flux-roadmap Decision 0006 rule 11 turned this story's prose
  acknowledgment of the datasource gap into a program rule, and the vendor datasource surface
  itself is the `vendor-datasources` epic
  ([C-511](C-511-vendor-datasources-epic.md),
  [vendor-datasource-declarations.md](../designs/vendor-datasource-declarations.md)) — a hard
  predecessor of this wave for every datasource-declaring plugin.
