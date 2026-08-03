---
id: C-496
title: "Align the connector charter with the runtime axis"
pillar: Bridge
status: done
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [docs]
note: "historical two-placement charter alignment; superseded by C-507 when flux-roadmap Decision 0001 made Exchange the sole official executor"
---

# Align the connector charter with the runtime axis

## Goal

Make the accepted cross-repository direction authoritative in this repository: all integrations are
connectors, and rich protocols use declared runtime artifacts locally or through Exchange.

## Acceptance

- [x] `AGENTS.md`, `README.md`, `docs/vision.md`, `docs/roadmap.md` and current design records no
      longer direct technology adapters back into `../flux/plugins`.
- [x] The docs distinguish future ownership from shipped capability: outbound operation dispatch is
      still HTTP-only, generated socket channels are the delivered rich-protocol slice, and no
      general rich operation executor is claimed to exist.
- [x] The roadmap links the complete C-495 program and its Flux/Exchange counterparts.
- [x] The engineering and customer changelogs describe the direction without claiming a migrated
      adapter already ships.
- [x] Documentation links and the generated story board validate.

## Progress

- 2026-08-03: Done. The conflicting charter passages were replaced, current rich-channel versus
  future operation-runtime status is explicit, and all three repository programs are linked.

## Notes

- This is documentation and backlog alignment, not runtime behavior; no failing-first behavioral
  test applies.
- **Superseded after completion:** C-507 adopts flux-roadmap Decision 0001. The `done` status here
  records the earlier decision honestly; it does not authorize local Flux execution or
  local-versus-Exchange parity in the current program.
