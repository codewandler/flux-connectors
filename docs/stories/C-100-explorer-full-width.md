---
id: C-100
title: Render the explorer full-width
pillar: Surfaces
status: in-progress
priority: 3
design: docs/designs/explorer-ux.md
epic: explorer-ux
areas: [web]
note: "the largest visible gain for the smallest diff — VPDoc.vue:191 caps content at 688px, so 16 provider cards render in two columns on a page allowed to be 1440px"
---

# Render the explorer full-width

## Goal
Give the explorer the width the page already permits, so a 16-connector grid and an 88-row operation
list are scannable instead of columnar.

## Acceptance
- [x] `/explorer` renders across the full layout width. The prose pages are **unchanged** — the doc
      layout is right for paragraphs, and widening the overview would make it harder to read.
- [~] The provider grid yields **three** columns at a desktop viewport, not two. **Four moved to
      [C-103](C-103-explorer-information-density.md) by coordinator decision**, not dropped: a fourth
      track at 1025px needs a minimum of 244px, and `.card__head` measures 273px min-content because
      it does not wrap — measured independently at 273px by the reviewer and 274px by the
      implementor. Reaching four therefore requires restructuring the card header, which is C-103's
      work and was fenced away from this story. Requiring it here was a scoping error in the
      dispatch, not a shortfall in the implementation. `minmax(320px, 1fr)` is retained; the
      arithmetic is recorded in `CatalogExplorer.vue`.
- [x] The filter bar sits on **one row** at a desktop viewport rather than wrapping to two or three.
- [~] Responsive down to a phone: the grid collapses to one column and the filter bar wraps, both
      verified. Horizontal overflow splits into two distinct defects and they must not be conflated:
      - **1280 and 1366px — introduced by this story, and fixed in it.** Widening to two ~424px
        columns pushed the hosts cell's unbreakable inline run off the page: 29px at 1280 and 8px at
        1366, against **0px at the merge base**, measured independently by the implementor and the
        reviewer. `.card__hosts` now wraps. **The after-measurement was not reproduced** — no browser
        is available in the coordinator's environment — so what is verified is the mechanism, pinned
        by `a card fact holding several values can break between them`. A human at 1280px settles it.
      - **Phone — pre-existing and untouched.** Base and branch overflow *identically* (193px at
        390px per the implementor; 83px at Chrome's ~485px headless floor per the reviewer — the
        equality is the load-bearing part). Cause is `<ul class="list">` being a grid, so each
        `OperationRow` needs `min-width: 0`. That file is C-103's.
- [x] The page still has a usable in-page structure. `outline: [2, 2]` currently drives the right-hand
      outline; if the chosen layout drops it, the story says what replaces it — the two `<h2>` anchors
      (`#providers`, `#operations`) are linked from elsewhere and must keep working.
- [x] `npm run build` passes and the deployed base path still resolves (`base: '/flux-connectors/'`).

## Progress

**The route taken: neither `layout: page` nor a CSS override — `aside: false`.**

The 688px is not a width the theme exposes as a variable. It is one rule,
`.VPDoc.has-aside .content-container { max-width: 688px }`, and its selector says what the cap is
*for*: it reserves room for the right-hand outline. So the page that has to be wide is the page that
must not carry an outline, and the theme already has frontmatter for that. One key, no rule fighting
the theme, and everything else the doc layout gives a Markdown page is kept.

- **`layout: page` was rejected.** It is the shorter diff and it is advertised as one line, but
  `VPPage.vue` renders a bare `<Content />` with no `vp-doc` class and no padding, so this page's
  heading, paragraph and warning callout would lose the site's prose styling and be flush to the
  viewport edge. Restoring that costs more CSS than the cap did.
- **A scoped `max-width` override was rejected** because it treats the symptom. It would leave the
  aside rendered and then overlap or crowd it, and it is a rule the next VitePress minor can
  silently win against; `aside: false` is the theme's own supported switch.

**What replaces the outline.** It listed exactly two entries, which is not navigation. Both headings
are still rendered by `CatalogExplorer.vue` with explicit `id="providers"` and `id="operations"`, so
every inbound `#providers` / `#operations` link still resolves — asserted in
`web/test/explorer.test.mjs`.

**Measured in Chrome against the built site** (`.content-container` width, provider columns, filter
bar rows), before → after:

| viewport | content column | provider columns | card rows | filter bar |
|---|---|---|---|---|
| 1920 | 688 → **1025** | 2 → **3** | 8 → **6** | 2 rows → **1 row** |
| 1440 | 688 → **1025** | 2 → **3** | 8 → **6** | 2 rows → **1 row** |
| 1280 | 609 → **865** | 1 → **2** | 16 → **8** | 2 rows |
| 390 | 327 (unchanged below 960px) | 1 | 16 | 4 rows |

**The grid minimum is kept at 320px, and that is why the four-column item is unticked.** `auto-fit`
fits `floor((width + gap) / (min + gap))` tracks, so a fourth column on 1025px needs a minimum of
244px or less. A card is `min` less 40px of padding, and the widest card header — vendor name, id
and status badge on one unwrapped flex line — has a min-content width of **274px**, so a card needs
**314px** before the badge escapes its own border. At a 240px minimum, twelve of the sixteen cards
overflow, visibly, into their neighbour. Four columns is therefore a **card** change and not a grid
change: one `flex-wrap: wrap` on `.card__head` in `ProviderCard.vue` unlocks it, and that file is
C-103's. 320px is the smallest round number above the 314px floor.

**The phone item is unticked for a defect this story did not introduce.** At 390px the page
overflows horizontally by 193px, identically before and after — `<ul class="list">` is a grid, so
each `OperationRow` gets `min-width: auto` and a long unbreakable `<code>` request path (for example
`/v0/{baseId}/{tableIdOrName}/{recordId}`) pushes the track past the viewport. The fix is
`min-width: 0` on `.row`, in `OperationRow.vue` — also C-103's.

## Notes
- Two routes, and the choice is the story's real content: **`layout: page`** in `explorer.md`
  frontmatter (drops sidebar and outline, gives full width, one line of diff), or a **scoped CSS
  override** of the doc layout's `max-width` under a page-specific class (keeps sidebar and outline,
  costs a rule that fights the theme). Pick one, record why.
- Verification here is a build plus looking at the page. Say that rather than inventing a unit test
  for CSS — `web/test/explorer.test.mjs` covers the pure selectors, and layout is not one.
