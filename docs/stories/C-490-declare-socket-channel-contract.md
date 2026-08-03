---
id: C-490
title: "Declare and publish the socket channel contract"
area: Spec
status: done
priority: 1
design: docs/designs/channel-bindings.md
epic: generated-websocket-channels
note: "SocketConnectSpec, channel-scoped query config, event wire values, payload root, full auth/config/event/channel manifest and catalogue facts"
---

# Declare and publish the socket channel contract

## Goal

Give a host every validated fact required to compose and route a generic RFC 6455 connector channel
without reading provider TOML.

## Acceptance

- [x] Failing-first loader tests refuse socket/connect transport mismatches, non-relative paths,
      forbidden fixed headers, invalid subprotocols, unbound query settings and payload root mixed
      with field projection.
- [x] `SocketConnectSpec` declares relative path, query values, fixed headers, auth requirements and
      optional subprotocols for a socket binding.
- [x] Configuration binding admits `channel.<binding>.query.<parameter>` and never affects operation
      request composition.
- [x] `EventDecl::wire_value` and `ChannelBinding::payload_root` round-trip through provider loading.
- [x] Manifest and `connector-catalog` publish complete auth, config, event, channel and socket fields;
      projection census tests fail if a field is dropped.
- [x] Focused tests, format and clippy are green.

## Progress

- 2026-08-02: socket/connect validation, channel query binds, wire values, root payloads and complete
  manifest/catalogue projections are implemented. The clean full workspace gate is running.
- 2026-08-03: the complete gate and v0.17.0 publish closure are green.
