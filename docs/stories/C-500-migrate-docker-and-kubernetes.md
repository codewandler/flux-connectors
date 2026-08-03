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

Replace the Docker and Kubernetes native plugins with connector bundles executed by Exchange in
single-tenant mode or inside an explicit per-tenant isolation boundary.

## Acceptance

- [ ] The connector definitions cover the current operation/event surfaces and accurately declare
      Unix/TCP/socket, process and streamed behavior rather than flattening it into fake HTTP calls.
- [ ] Endpoint and credential references remain host-resolved; kubeconfig, client keys and daemon
      authority never become ordinary operation arguments.
- [ ] Logs, watches, exec and port-forward have bounded stream/cancellation/lease contracts.
- [ ] Exchange refuses locally executing runtime plans in a shared process and admits them only in
      its single-tenant mode or through a per-tenant isolated worker; Flux has no local fallback.
- [ ] Conformance and migration documentation unblock removal of both Flux plugin crates.

## Progress

- (not started)

## Notes

- This is migration wave 2 after C-499. Exchange may reuse published generic runtime libraries, but
  official execution does not move into the Flux CLI or its release pipeline.
