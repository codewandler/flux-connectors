---
id: C-495
title: "All official integrations become connectors (epic)"
pillar: Bridge
status: in-progress
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [connector-spec, connector-pack, providers, runtime]
note: "EPIC — every official integration is connector-owned and Exchange-executed; migrate all 18 Flux adapters without a local Flux runtime or fallback"
---

# All official integrations become connectors

## Goal

Move every official integration surface and vendor-specific runtime adapter into flux-connectors so
Exchange executes it under tenant-derived authority and Flux reaches it only through the embedded
Exchange client.

## Acceptance

- [ ] The charter and public documentation state that protocol richness selects a runtime; it does
      not select a different repository or extension model (C-496).
- [ ] A connector can bind operations to every declared runtime and can name an immutable runtime
      artifact without putting execution behavior into interpreted TOML (C-497, C-498, C-504).
- [ ] Every integration adapter measured in `../flux/plugins` has one migration story and no adapter
      is omitted or assigned to two waves (C-499…C-503, checked by C-505).
- [ ] C-505 establishes the checked inventory and frozen legacy-plugin-versus-Exchange conformance
      ratchet before C-499 begins; every wave extends it before Flux removes a native crate.
- [ ] HTTP remains only one runtime. Docker, Kubernetes, SQL, observability, secret stores and other
      stateful/rich protocols are represented in the same catalogue and are invocable through
      Exchange with the required isolation.
- [ ] No official connector executes in Flux and no vendor adapter, plugin or connector-runtime
      fallback remains in its release pipeline after the migration.

## Progress

- 2026-08-03: Filed the cross-repository program and complete current plugin mapping. C-405 and
  C-489…C-492 are delivered foundations; in-flight worktree C-494 supplies instance-aware host
  ports; implementation stories C-497…C-505 are filed.
- 2026-08-03: C-507 adopted flux-roadmap Decision 0001 and superseded the proposed local Flux
  placement before runtime implementation began.

## Notes

- Cross-repository source: `../flux-roadmap/decisions/0001-exchange-executes-official-integrations.md`.
- Exchange counterpart: X-111. Flux counterpart: C-500; Flux C-493…C-499 are already consumed by
  pending maintenance and release worktrees.
