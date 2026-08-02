---
id: C-490
title: "Declare and publish the socket channel contract"
area: Spec
status: in-progress
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

- [ ] Failing-first loader tests refuse socket/connect transport mismatches, non-relative paths,
      forbidden fixed headers, invalid subprotocols, unbound query settings and payload root mixed
      with field projection.
- [ ] `SocketConnectSpec` declares relative path, query values, fixed headers, auth requirements and
      optional subprotocols for a socket binding.
- [ ] Configuration binding admits `channel.<binding>.query.<parameter>` and never affects operation
      request composition.
- [ ] `EventDecl::wire_value` and `ChannelBinding::payload_root` round-trip through provider loading.
- [ ] Manifest and `connector-catalog` publish complete auth, config, event, channel and socket fields;
      projection census tests fail if a field is dropped.
- [ ] Focused tests, format and clippy are green.

## Progress

- 2026-08-02: socket/connect validation, channel query binds, wire values, root payloads and complete
  manifest/catalogue projections are implemented. The clean full workspace gate is running.
