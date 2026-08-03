---
id: C-500
title: "Migrate Docker and Kubernetes into infrastructure connectors"
pillar: Connector
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [providers, runtime, infrastructure]
note: "preserve Unix-socket, kubeconfig/in-cluster, watch/log/exec and port-forward behavior through declared rich runtimes; Exchange requires tenant isolation"
---

# Migrate Docker and Kubernetes into infrastructure connectors

## Goal

Replace the Docker and Kubernetes native plugins with connector bundles whose rich local adapters can
also be hosted by Exchange inside an explicit tenant isolation boundary.

## Acceptance

- [ ] The connector definitions cover the current operation/event surfaces and accurately declare
      Unix/TCP/socket, process and streamed behavior rather than flattening it into fake HTTP calls.
- [ ] Endpoint and credential references remain host-resolved; kubeconfig, client keys and daemon
      authority never become ordinary operation arguments.
- [ ] Logs, watches, exec and port-forward have bounded stream/cancellation/lease contracts.
- [ ] Local execution uses Flux's guarded substrate; hosted execution is refused in a shared process
      and succeeds only in Exchange's single-tenant or per-tenant isolated placement.
- [ ] Conformance and migration documentation unblock removal of both Flux plugin crates.

## Progress

- (not started)

## Notes

- Reuses Flux C-394/C-397/C-399/C-435 rather than creating connector-owned IO primitives.
