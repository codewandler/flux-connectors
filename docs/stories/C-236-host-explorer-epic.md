---
id: C-236
title: "The host's explorer — an operator console, then convergence on the site's components (epic)"
pillar: Surfaces
status: ready
priority: 2
design: docs/designs/host-explorer.md
epic: host-explorer
areas: [host, web]
note: "EPIC — filed 2026-07-31 after the owner ran the app and found the built-in explorer visibly weaker than the public site's. The cause is recorded: no story ever asked for more, and explorer-ux.md is scoped exclusively to web/"
---

# The host's explorer — an operator console, then convergence (epic)

## Goal

Make the page an operator actually works in as good as the page they read, without moving credential
capture or execution into a surface that is forbidden to have them.

## Why now

The owner ran the app and reported it working but visibly weaker than the public documentation site.
The measured gap is **355 lines** against **2,434 lines of Vue plus 771 lines of selectors**, and the
cause is not neglect: [C-203](C-203-connectors-api-skeleton.md) scoped this page as *"no UI beyond
what proves it"*, and every change since has been a rider on a backend story. There is no written
decision to keep it minimal — the UI has simply never been the subject of a story.

## Acceptance

- [ ] The console is usable at catalogue scale — [C-237](C-237-the-host-explorer-is-a-console.md).
- [ ] The host mounts the published explorer components rather than restating them —
      [C-238](C-238-the-host-mounts-the-explorer-components.md).
- [ ] A behavioural change to the page can ship with a failing-first test —
      [C-239](C-239-a-test-harness-for-the-host-page.md).
- [ ] **The asymmetry survives.** Credential capture and execution stay in this host and never reach
      a component `web/` also mounts. [C-147](C-147-explorer-runs-an-operation.md) forbids the public
      site collecting a credential or implying a live call; that is a structural property, not a
      preference, and a redesign that blurs it is a regression however much better it looks.
- [ ] **No second emitter of `catalog.json`.** The design records the two routes and takes the
      reversible one; producing a second document of the same shape is the drift this repository
      exists to prevent.
- [ ] The gate is green, and the web gate is run in its documented order —
      `cd web && npm ci && npm run build && npm test`. The build **must** precede the test.

## Notes

- **Do not diverge from [C-99](C-99-explorer-ux-epic.md).** The site's own fleet-scale epic owns
  width, services, shareable views and density for `web/`. The filter and density thinking is shared;
  duplicating it in two places is the failure this epic is most likely to cause.
- Two open stories already land in `crates/connectors-api/src/index.html` —
  [C-225](C-225-a-config-field-cannot-declare-a-closed-set-of-values.md) wants a closed value set
  rendered as a choice, and [C-226](C-226-one-credential-cannot-be-shared-by-two-connectors.md) wants
  a credential satisfied by another connection shown as such. Leave room for both.
- Sequencing against the ten-story quality run: C-237 needs nothing. C-238 needs
  [C-158](C-158-typescript-catalogue-types-drift.md) and
  [C-191](C-191-publish-the-explorer-components.md), in that order — C-191's other blocker C-205 is
  already done.
