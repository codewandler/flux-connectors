---
id: C-535
title: "Adopt Decision 0022 across the repository contract"
pillar: Bridge
status: ready
priority: 0
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [docs]
note: "Contract correction: amend the vision north star, close C-10/C-15 as superseded, restate Path C and the artifact table for the catalog-artifact destination"
---

# Adopt Decision 0022 across the repository contract

## Goal

Make `../flux-roadmap/decisions/0022-connectors-compile-to-a-catalog-artifact.md` authoritative
in this repository's own contract documents before any dependent implementation begins, following
the C-507 precedent.

## Acceptance

- [ ] `docs/vision.md` amends the north star: request shaping is closed declarative data evaluated
      by the resolver; behaviour (composition, retry, saga, approval) stays in Flux-Lang at the
      Flux layer. The amendment records the owner direction and date, keeping the history the way
      the C-201 amendment did.
- [ ] `AGENTS.md` names the catalog artifact as the compile destination and gives Decision 0022
      precedence language consistent with C-507's for Decision 0001.
- [ ] `README.md` ("What the compiler produces", "Design in one screen", limitations) and
      `docs/integrating-with-flux.md` (Path C, the artifact table, Gap 5) describe the destination
      without claiming undelivered capability.
- [ ] C-10 and C-15 are closed as superseded with honest historical notes (the C-496 pattern); the
      `.flux`-module half of C-41/`docs/designs/connector-bundle.md` is annotated as superseded.
- [ ] The generated board and documentation checks pass. This is a contract correction; no
      failing-first behavioral test applies and no new runtime capability is claimed.

## Progress

- (not started)

## Notes

- Do not delete or weaken repository-local safety rules while reconciling; Decision 0022 changes
  the compile target, not the secrets boundary, the offline guarantee, or the fail-closed rules.
