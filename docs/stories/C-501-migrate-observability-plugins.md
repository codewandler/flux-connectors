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
