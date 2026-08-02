---
id: C-483
title: "Asterisk ARI ships as a spec-generated REST connector (epic)"
pillar: Agent
status: in-progress
priority: 1
design: docs/designs/asterisk-ari-rest.md
epic: asterisk-ari-rest
areas: [providers, openapi, asterisk]
note: "EPIC — 108 REST operations from official ARI Swagger; event WebSocket deferred to future channels"
---

# Asterisk ARI ships as a spec-generated REST connector

## Goal

Ship the complete non-WebSocket Asterisk ARI surface from its first-party API descriptions through
the normal `flux-connectors` pipeline, with no Asterisk plugin or runtime invention in Flux.

## Acceptance

- [ ] C-484 vendors and deterministically normalizes the official ARI descriptions with an exact
      109-total / 108-REST / 1-WebSocket census.
- [ ] C-485 selects all 108 REST operations into one callable Asterisk connector and proves request
      composition through the scoped provider gate.
- [ ] Event WebSocket handling is absent and explicitly deferred to future channel work.
- [ ] C-486 regenerates the whole catalogue, passes every repository gate, and cuts the immediate
      new-provider release.
- [ ] The separate Flux correction removes the entire Asterisk plugin rather than retaining AMI.

## Progress

- 2026-08-02: owner corrected the repository boundary: ARI is a simple REST interface and must be
  sourced from its API specs inside `flux-connectors`; eventing is deferred because channels need
  more design work.
- 2026-08-02: `python3` over the 11 first-party documents measured 76 paths and 109 operations:
  108 ordinary HTTP operations and one `upgrade = "websocket"` operation.

## Notes

- This design explicitly supersedes the old charter example that classified all Asterisk work as a
  Flux plugin regardless of transport.
