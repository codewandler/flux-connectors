---
id: C-485
title: "Generate the complete Asterisk ARI REST provider"
pillar: Connector
status: done
priority: 1
design: docs/designs/asterisk-ari-rest.md
epic: asterisk-ari-rest
areas: [providers, connector-spec, connector-flux, asterisk]
note: "all 108 non-WebSocket operations; Basic auth and configured deployment authority; no eventing"
---

# Generate the complete Asterisk ARI REST provider

## Goal

Compile every non-WebSocket ARI operation into callable connector artifacts using the repository's
existing spec selection and HTTP execution path.

## Acceptance

- [x] A failing-first provider test proves Asterisk is absent before implementation.
- [x] `providers/asterisk.toml` selects exactly all 108 normalized REST operations and selects no
      invented operation; the WebSocket upgrade is absent.
- [x] Basic username/password configuration and the deployment endpoint authority compose through
      the ordinary connector-pack configuration and egress guards.
- [x] Every operation has reviewed risk, idempotency, effects and response provenance; the complete
      surface is catalogued while model exposure remains bounded.
- [x] `build --provider asterisk`, `diff --provider asterisk`, the request-composition rehearsal,
      workspace build, no-fail-fast tests, clippy and formatting produce the scoped-provider evidence
      required by `AGENTS.md`.

## Progress
- 2026-08-03: C-30 subsequently narrowed the published REST surface to 96 operations. Twelve
  array-query operations remain present in the normalized source but are withheld by exact,
  reason-bearing provider deferrals until their serialization convention is modelled.

- 2026-08-02: scoped after the owner explicitly deferred eventing and rejected a cross-repository
  plugin implementation.
- 2026-08-02: implementation started from `36a3a3d`; C-484 owns the normalized source document and
  this story owns only the provider, provider-specific proof, and scoped generated artifacts.
- 2026-08-02: independent scoped verification measured 109 source operations, 108 selected REST
  operations and one deferred WebSocket; all 108 compose through request rehearsal and
  `diff --provider asterisk` reports 111 artifacts current.
- 2026-08-02: done after the full workspace no-fail-fast suite, build, Clippy and formatting gates
  all passed on the integrated catalogue.

## Notes

- C-484 must land first because its normalized spec is this story's only operation source.
