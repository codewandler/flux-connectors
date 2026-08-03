---
id: C-507
title: "Adopt the Exchange-only execution path for official integrations"
pillar: Bridge
status: done
priority: 1
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [docs, runtime, migration]
note: "Decision 0001 makes Exchange the sole executor for official external integrations; this supersedes local Flux execution and local-versus-hosted parity throughout C-495…C-505"
---

# Adopt the Exchange-only execution path for official integrations

## Goal

Make the cross-repository decision in `../flux-roadmap` authoritative here: flux-connectors owns
official integration declarations, runtime plans and vendor-specific artifacts, while Exchange is
their only supported execution boundary and Flux embeds only the Exchange client.

## Acceptance

- [x] `AGENTS.md` gives accepted `../flux-roadmap` decisions precedence for cross-repository
      architecture and states the Exchange-only execution boundary without weakening repository-local
      safety and implementation rules.
- [x] The accepted integration design and C-495 epic remove local Flux execution, optional Exchange
      placement and local-versus-hosted parity from the destination.
- [x] C-497…C-504 make declarations, artifacts, plans and migration waves target Exchange without a
      Flux runtime consumer or vendor/plugin fallback.
- [x] C-505 becomes the atomic prerequisite that establishes the inventory and legacy-plugin-versus-
      Exchange conformance ratchet before the first migration wave; C-499…C-503 consume and extend it
      in the fixed collaboration → infrastructure → observability → data/secrets → remaining order.
- [x] C-496 remains an honest historical completion and explicitly names this later supersession;
      current repository narrative and changelogs no longer publish its old two-placement outcome.
- [x] The generated board and documentation checks pass. This is a contract correction, so no
      failing-first behavioral test applies and no new runtime capability is claimed.

## Progress

- 2026-08-03: Adopted flux-roadmap Decision 0001 and reconciled the complete C-495…C-505 program
  before any dependent runtime implementation begins.

## Notes

- Cross-repository source of truth: `../flux-roadmap/decisions/0001-exchange-executes-official-integrations.md`.
- The existing `connectors-api` binary remains a reference/development host for delivered HTTP
  seams; it is not a supported official integration placement and does not compete with Exchange.
