---
id: C-491
title: "Compose redacted zero-I/O WebSocket channel plans"
area: Host
status: in-progress
design: docs/designs/channel-bindings.md
epic: generated-websocket-channels
note: "connector_pack::channel_plan resolves declared config/auth into exact ws URL, headers and subprotocols without a client, DNS or socket"
---

# Compose redacted zero-I/O WebSocket channel plans

## Goal

Let a host turn catalogue facts and tenant-bound ports into an exact WebSocket handshake plan without
performing I/O or leaking credential-bearing material through diagnostics.

## Acceptance

- [ ] Failing-first tests pin exact path/query encoding, optional defaults, Basic auth and fixed
      headers and prove `Debug`/errors never contain secret values.
- [ ] `channel_plan` returns URL, headers, subprotocols, host contract and routing facts with no HTTP
      client, resolver, runtime or socket dependency.
- [ ] Missing config/auth and final authority drift refuse with addresses/names, never values.
- [ ] Package fence, focused tests, format and clippy are green.

## Progress

- 2026-08-02: `channel_plan` composes redacted URL/auth plus the declared-origin and complete routing
  facts without a client, resolver, runtime or socket. Exact Asterisk and failure tests are green.
