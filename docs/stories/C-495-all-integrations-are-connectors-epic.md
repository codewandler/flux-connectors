---
id: C-495
title: "All official integrations become connectors (epic)"
pillar: Bridge
status: in-progress
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [connector-spec, connector-pack, providers, runtime]
note: "EPIC — replace the generated-HTTP/native-plugin split with one connector catalogue across http, socket, process, container, plugin and remote runtimes; migrate all 18 Flux integration adapters"
---

# All official integrations become connectors

## Goal

Move every official integration surface and every vendor-specific runtime adapter into
flux-connectors, while Flux retains only generic guarded mechanisms and Exchange can host the same
connector remotely without receiving caller-selected authority.

## Acceptance

- [ ] The charter and public documentation state that protocol richness selects a runtime; it does
      not select a different repository or extension model (C-496).
- [ ] A connector can bind operations to every declared runtime and can name an immutable runtime
      artifact without putting execution behavior into interpreted TOML (C-497, C-498, C-504).
- [ ] Every integration adapter measured in `../flux/plugins` has one migration story and no adapter
      is omitted or assigned to two waves (C-499…C-503, checked by C-505).
- [ ] The migrated connectors pass one local/hosted conformance contract before Flux removes their
      native crates; catalogue presence alone is not parity (C-505 and Flux C-502…C-506).
- [ ] HTTP remains only one runtime. Docker, Kubernetes, SQL, observability, secret stores and other
      stateful/rich protocols are represented in the same catalogue and are invocable through
      Exchange with the required isolation.

## Progress

- 2026-08-03: Filed the cross-repository program and complete current plugin mapping. C-405 and
  C-489…C-492 are delivered foundations; in-flight worktree C-494 supplies instance-aware host
  ports; implementation stories C-497…C-505 are filed.

## Notes

- Cross-repository source: `../flux/docs/designs/ecosystem.md`.
- Exchange counterpart: X-111. Flux counterpart: C-500; Flux C-493…C-499 are already consumed by
  pending maintenance and release worktrees.
