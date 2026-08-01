---
id: C-422
title: "A patch cannot drop a parameter, so one vendor endpoint becomes a 38-argument tool with 17 synonyms"
pillar: Spec
status: ready
priority: 2
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec]
note: "measured by C-416 on 2026-08-01 — THE one place hand-authoring beat patching. babelforce-call-list goes from 11 curated query parameters to 38, of which 17 are exact synonyms the vendor documents alongside their own aliases"
---

# A patch cannot drop a parameter, so one vendor endpoint becomes a 38-argument tool with 17 synonyms

## Goal
Let a patch omit a parameter the document declares, so a curated operation stays curated when it
moves from hand-authored to spec-backed.

## Acceptance
- [ ] A patch can omit named parameters of a selected operation, by name and position.
- [ ] **Omission is explicit and never inferred.** A parameter the vendor declares and the connector
      drops is a decision, so it is written down and survives regeneration — the same argument
      `Patch` uses for having no `hide` at the operation level applies one level down and lands the
      opposite way *because the operation is already selected*: the author has stated intent about
      this endpoint, and is narrowing it rather than opting out of review.
- [ ] Omitting a **required** parameter is refused — that composes a request the vendor rejects, and
      is the one case where silence would produce a runtime failure rather than a wide tool.
- [ ] Omitting a parameter the document does not declare is a loud error, exactly as an unmatched
      `ParamPatch` correction already is.
- [ ] `babelforce-call-list` comes back to a curated argument list, and the story records the before
      and after counts.

## Progress
- (not started)

## Notes
- **Measured, not anticipated.** C-416's conversion of babelforce cost **293 → 54 declaration lines
  (32.6 → 6.0 per operation)** with **zero parameter patches needed** — the document's descriptions
  and schemas beat the hand transcription everywhere except here. This is the single counter-example,
  and it is a capability gap rather than a cost problem: an `omit` list is ~4 lines against 24.
- **Wanted before C-417.** At 397 operations the same shape recurs across the catalogue, and a tool
  surface full of synonym arguments degrades the model that has to choose between them — which is the
  same argument that motivated C-413's exposure tier, one level down.
- Sequenced with [C-6](C-6-overlay-layer.md), which owns the other open half of the overlay (adding a
  parameter the document omits). These are the two directions of one capability and may well land
  together.
