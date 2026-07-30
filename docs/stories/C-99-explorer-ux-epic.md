---
id: C-99
title: "The explorer at fleet scale — wider, and legible at 16 connectors (epic)"
pillar: Surfaces
status: ready
priority: 3
design: docs/designs/explorer-ux.md
epic: explorer-ux
areas: [web]
note: "EPIC — the explorer was designed for 6 providers and 25 operations; it now indexes 16, 18 services and 88. VPDoc caps content at 688px, so the provider grid renders exactly two columns"
---

# The explorer at fleet scale — wider, and legible at 16 connectors (epic)

## Goal
Make the connector explorer usable at the size the catalogue actually is, without losing the honest
account of what does not work that it was built to give.

## Acceptance
- [ ] The explorer renders full-width — [C-100](C-100-explorer-full-width.md).
- [ ] Services are a visible, filterable dimension — [C-101](C-101-services-in-the-explorer.md).
- [ ] A filtered view is a shareable URL, and the list can be sorted —
      [C-102](C-102-shareable-explorer-views.md).
- [ ] Cards and rows carry the density a 16×88 catalogue needs —
      [C-103](C-103-explorer-information-density.md).
- [ ] **The scope-based honesty survives.** A redesign that reports "0 of 88 operations working", or
      that filters on `works` rather than on whether an operation owns a defect, is a regression no
      matter how it looks. The reasoning is in `CatalogExplorer.vue`'s header comment and in the
      design; it is restated in whatever replaces them.
- [ ] `npm run build` passes with `ignoreDeadLinks: false`, and `npm test` stays green.

## Progress
- Not started. Filed 2026-07-30 from a request to make the explorer wider with better UX and UI.

## Notes
- **The width is a measured fact, not a preference.** `VPDoc.vue:191` caps the content column at
  `688px` while `--vp-layout-max-width` is `1440px`, and the provider grid's `minmax(320px, 1fr)`
  therefore yields exactly two columns for sixteen connectors.
- **Deliberately excludes the inbound surface.** Events, channel bindings, config fields and graphs
  are in the IR but reach `catalog.json` only when [C-83](C-83-channel-binding-codegen.md) and
  [C-87](C-87-configuration-codegen.md) land. Rendering them is a later story; a story here that tried
  would block on codegen.
- The stale counts in the component comments ("all 25 operations", "the five with a problem") are
  [C-81](C-81-declared-counts-are-checked.md)'s class of defect, not this epic's.
