---
id: C-489
title: "Generate declarative WebSocket channels (epic)"
area: Connector
status: in-progress
priority: 1
design: docs/designs/channel-bindings.md
epic: generated-websocket-channels
note: "EPIC — declare generic RFC 6455 handshakes, publish complete host inputs, compose zero-I/O plans and generate Asterisk ARI's full event channel"
---

# Generate declarative WebSocket channels

## Goal

Make this repository authoritative for generic connector WebSocket handshakes and Asterisk ARI
events while preserving the compiler/host-library network fence.

## Acceptance

- [ ] C-490 adds the fail-closed socket/event/config IR and complete manifest/catalogue projections.
- [ ] C-491 adds zero-I/O redacted `connector_pack::channel_plan` composition from catalogue facts and
      tenant-bound ports.
- [ ] C-492 generates Asterisk `ari-events`, proves exact source/event censuses, regenerates
      coordinator-owned artifacts and closes the whole workspace gate.

## Progress

- 2026-08-02: filed from Flux master design
  `../flux-generated-connector-ws/docs/designs/generated-connector-websocket-channels.md`.
