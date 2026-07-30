---
id: C-103
title: Card and row density for a 16-connector, 88-operation catalogue
pillar: Surfaces
status: ready
priority: 5
design: docs/designs/explorer-ux.md
epic: explorer-ux
areas: [web]
note: "the UI treatment a tripled catalogue needs — and the story most at risk of removing the scope-based honesty by accident"
---

# Card and row density for a 16-connector, 88-operation catalogue

## Goal
Make 16 cards and 88 rows scannable, once C-100 has given them room.

## Acceptance
- [ ] `ProviderCard.vue` is legible at a glance: vendor, what it is for, operation count, auth
      scheme(s), hosts. Decide what earns its place at four-across and what moves to the provider page
      — a card that repeats everything is not denser, only wider.
- [ ] `OperationRow.vue` reads as a scannable row rather than a stacked block: id, vendor, method and
      path, risk and idempotency, and the defect badge where one applies.
- [ ] Method and risk are distinguishable **without relying on colour alone** — a `GET` and a
      `DELETE`, and `low` versus `destructive`, must be tellable apart in monochrome and by a
      colour-blind reader. Contrast meets WCAG AA against both the light and the dark theme, since the
      site ships both.
- [ ] The operation list has a visible grouping or sticky affordance so a reader 60 rows down still
      knows which connector they are in.
- [ ] **The scope-based presentation is preserved and restated.** `catalog` issues render once as a
      banner; `provider` issues on the card; `operation` issues as a badge with its reason. The
      headline stays "N with an operation-specific limitation" and never becomes "N of 88 working" —
      `works` is false for all 88 today and a working-count headline would misrepresent eighty
      operations that are exactly as designed.
- [ ] Empty and degenerate states are designed, not incidental: no results, a connector with one
      operation, an operation with no parameters.

## Progress
- Not started.

## Notes
- **Sequenced after [C-100](C-100-explorer-full-width.md).** Tuning density inside a 688px column and
  then again at full width is doing the work twice.
- The honesty constraint is the acceptance item most likely to be lost in a visual pass, which is why
  it is written as acceptance rather than left in a component comment. `CatalogExplorer.vue`'s header
  and `OperationList.vue`'s last filter carry the original reasoning — read both before changing
  either.
- If a proposed treatment needs data the catalogue does not publish, that is a codegen story
  ([C-83](C-83-channel-binding-codegen.md) / [C-87](C-87-configuration-codegen.md)), not a reason to
  invent a field here.
