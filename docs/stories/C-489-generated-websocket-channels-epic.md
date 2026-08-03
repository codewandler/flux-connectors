---
id: C-489
title: "Generate declarative WebSocket channels (epic)"
area: Connector
status: done
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

- [x] C-490 adds the fail-closed socket/event/config IR and complete manifest/catalogue projections.
- [x] C-491 adds zero-I/O redacted `connector_pack::channel_plan` composition from catalogue facts and
      tenant-bound ports.
- [x] C-492 generates Asterisk `ari-events`, proves exact source/event censuses, regenerates
      coordinator-owned artifacts and closes the whole workspace gate.

## Progress

- 2026-08-02: filed from Flux master design
  `../flux-generated-connector-ws/docs/designs/generated-connector-websocket-channels.md`.
- 2026-08-03: v0.17.0 is published from CI with all four crates, the site and host page green; the
  tagged GitHub release records the generated channel contract and Flux 0.54 compatibility seam.
