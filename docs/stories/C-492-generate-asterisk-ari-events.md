---
id: C-492
title: "Generate and integrate the Asterisk ARI event channel"
area: Connector
status: done
design: docs/designs/asterisk-ari-rest.md
epic: generated-websocket-channels
note: "ari-events /events, Basic auth, app + subscribeAll, exact PascalCase wire values, kebab local names, full schemas and raw payload"
---

# Generate and integrate the Asterisk ARI event channel

## Goal

Account for the official ARI `/events` WebSocket and every `Event` subtype as one generated channel
beside the already-shipped REST operations.

## Acceptance

- [x] `ari-events` declares `/events`, Basic auth, required connection `app`, optional default-false
      `subscribe_all` rendered as `subscribeAll`, discriminator `type` and raw payload.
- [x] Every official `Event` subtype maps exactly once from PascalCase wire value to lowercase-kebab
      local name and carries its full resolved schema.
- [x] Two-way operation census accounts for 108 REST plus one socket channel; `/events` is neither
      silently emptied nor emitted as REST.
- [x] Scoped generation is verified first; integration regenerates only coordinator-owned
      whole-catalogue artifacts and updates measured counts if they changed.
- [x] Full workspace build/test-no-fail-fast/clippy/fmt and final `connector-cli diff` are green.

## Progress

- 2026-08-02: Asterisk emits one `ari-events` binding and all 45 exact event subtypes; the two-way
  census accounts for 108 REST operations plus the socket. Full coordinator-owned artifacts were
  regenerated and the clean workspace gate is running.
- 2026-08-03: the complete gate and v0.17.0 publish closure are green.
