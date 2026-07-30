---
id: C-111
title: Ship the Fly.io Machines connector
pillar: Spec
status: in-progress
priority: 1
design:
epic: provider-fleet-2
areas: [providers, connector-flux]
note: "A deliberately narrow machine-lifecycle surface: nine typed operations, one named service, and no invented channel contract"
---

# Ship the Fly.io Machines connector

## Goal

Compile Fly.io's stable Machines REST surface into a small, honest connector that covers discovery
and machine lifecycle without importing Fly's entire platform API or claiming unsupported inbound
semantics.

## Acceptance

- [ ] A failing-first provider contract pins the `io.fly.api/machines:v1` service, bearer credential,
      verification operation, and exact nine-operation inventory.
- [ ] The provider exposes regions, machine list/get/events, and create/start/stop/restart/delete;
      creation accepts only the required image at `config.image` and every optional query/body control
      remains absent.
- [ ] Risk and idempotency are honest: reads are low/idempotent, lifecycle writes are high or
      destructive and non-idempotent.
- [ ] No Fly event, channel, graph, raw credential, token acquisition, or free-form remote expression
      is emitted.
- [ ] Every generated operation parses, analyzes, formats canonically, and the deterministic build
      and diff gates include all Fly artifacts.
- [ ] Public/provider counts and snapshots are regenerated from the actual build output.

## Progress

- Contract fixed from Fly's official Machines OpenAPI and documentation; implementation not started.

## Notes

- The public Machines event-list endpoint has no durable cursor, so it cannot satisfy this repo's
  poll-channel contract. Fly's partner Extensions webhook is not a general connector surface.

