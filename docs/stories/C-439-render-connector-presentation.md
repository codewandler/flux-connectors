---
id: C-439
title: "Render a connector as itself — the mark and the links, in both the explorer and the host"
pillar: Surfaces
status: backlog
priority: 3
design: docs/designs/connector-presentation.md
epic: connector-presentation
areas: [web, connectors-api]
note: "the point of the epic — 54 cards that today differ by a name and a sentence. Two surfaces render the catalogue and both must learn it, and C-408's rule applies: a connector that declares no logo is not a connector rendered as broken"
---

# Render a connector as itself — the mark and the links, in both the explorer and the host

## Goal
Show the declared resources and the mark where a person actually chooses a connector, so the listing
individualises rather than repeating a name in 54 rows.

## Acceptance
- [ ] The public explorer renders a connector's resources and, if C-437 lands one, its mark.
- [ ] The host's operator console renders the same facts from the same document. Two surfaces read
      this catalogue — `web/` and `crates/connectors-api` — and a fact rendered in one and not the
      other is the drift [C-236](C-236-host-explorer-epic.md) exists to close.
- [ ] **Absence renders as absence, not as a defect.** A connector declaring no logo and no resources
      is not broken, and must not get a placeholder that reads as an error or a danger colour. This is
      [C-408](C-408-components-cannot-say-unpublished.md)'s rule exactly, and its `Published<T>`
      mechanism is already in `web/data/catalog.mts` — use it rather than inventing a second way.
- [ ] **A resource link is rendered as a link and never fetched, prefetched or previewed.** A listing
      that hits 54 vendors' servers when a page opens is a third-party-request surprise for whoever
      deployed it.
- [ ] `web/test/explorer.test.mjs` covers the new states — present, absent, and a resource list with
      a kind the site does not specially render.
- [ ] `web/`'s own rendering of the full catalogue is otherwise unchanged, measured the way C-408
      measured it: rebuild the base and compare, rather than asserting it.

## Progress
- (not started)

## Notes
- **Blocked on [C-436](C-436-connector-resources.md)**, and on [C-437](C-437-decide-the-logo.md) only
  for the mark — the links are worth rendering on their own and should not wait for the licensing
  decision.
- The site has exactly one dependency by design. C-408 solved a similar problem by testing pure
  functions and asserting components route through them rather than pulling in a Vue test renderer;
  follow that rather than adding a dependency.
- If a mark is hotlinked rather than vendored, this is where the third-party-request consequence
  actually lands on a user — say so in the component docs, not only in the design.
