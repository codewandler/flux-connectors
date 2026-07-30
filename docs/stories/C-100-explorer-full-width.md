---
id: C-100
title: Render the explorer full-width
pillar: Surfaces
status: done
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
- [x] The provider grid yields **four** columns at a desktop viewport, not two. It was moved to
      C-103 as unreachable, then reached here after the user lifted the card fence: the blocker was
      never the grid but the 273–274px min-content of an unwrapping `.card__head`, which put a 314px
      floor under a card. One `flex-wrap: wrap` removes the floor — the badge drops to its own line
      on a narrow card instead of escaping the border — and the track minimum is **re-tuned from
      320px to 240px**, which is the stated minimum. Measured: four 244px columns from 1440px up,
      three from 1180, two from 768, one on a phone.
- [x] The filter bar sits on **one row** at a desktop viewport rather than wrapping to two or three.
- [x] Responsive down to a phone: the grid collapses to one column, the filter bar wraps, and
      **nothing overflows horizontally** — 0px at every width measured from 390 to 2560. Getting
      there meant two distinct defects, and the distinction is kept because it is the reason one of
      them was nearly shipped:
      - **1280 and 1366px — introduced by this story, and fixed in it.** Widening to two ~424px
        columns pushed the hosts cell's unbreakable inline run off the page: 29px at 1280 and 8px at
        1366, against **0px at the merge base**, measured independently by the implementor and the
        reviewer. `.card__hosts` now wraps. **The after-measurement has now been reproduced** in
        headless Chrome against the built site, closing the caveat this line used to carry: a sweep
        of ten viewport widths from 390 to 2560 finds **0px of overflow and zero offending elements
        at every width from 768 up**, 1280 and 1366 included, and the hosts `<code>` no longer
        appears among the offenders at any width. The mechanism is pinned independently of the
        pixels by `a card fact holding several values can break between them`.
      - **Phone — pre-existing, and now fixed here too.** This one predates the story: base and
        branch overflowed *identically* (193px at 390px per the implementor; 83px at Chrome's ~485px
        headless floor per the reviewer — the equality is what proved it was not ours). It was
        deferred to C-103 on that basis, then fixed here once the user lifted the fence.
        `<ul class="list">` is a grid, so every `OperationRow` was held open at its longest request
        path; `min-width: 0` on `.row` lets the track shrink and `overflow-wrap` lets the path
        break. **193px → 0px at 390px.**
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

**Post-fix overflow sweep**, headless Chrome against the built site at `c355bee`, ten widths. This is
the measurement the rework commit recorded as not reproduced; it is reproduced here. `offenders`
counts elements whose right edge sits outside `documentElement.clientWidth`.

| viewport | content | cols | card | filter rows | overflow | offenders |
|---|---|---|---|---|---|---|
| 390 | 327 | 1 | 327 | 4 | 193 | 585, all `OperationRow` — pre-existing, C-103 |
| 768 | 689 | 2 | 336.5 | 2 | **0** | – |
| 960 | 545 | 1 | 545 | 2 | **0** | – |
| 1180 | 765 | 2 | 374.5 | 2 | **0** | – |
| 1280 | 865 | 2 | 424.5 | 2 | **0** (was 29) | – |
| 1366 | 951 | 2 | 467.5 | 2 | **0** (was 8) | – |
| 1440 | 1025 | 3 | 331 | 1 | **0** | – |
| 1600 | 1025 | 3 | 331 | 1 | **0** | – |
| 1920 | 1025 | 3 | 331 | 1 | **0** | – |
| 2560 | 1025 | 3 | 331 | 1 | **0** | – |

**Final sweep**, after the three min-content fixes, same rig. The `filter rows` column in the sweep
above was measuring the wrong thing — it counted distinct bottom edges across *both* `.filters` bars
on the page, the Flux core one included, so it never read below 2; scoped to the operation list's own
bar, the numbers are:

| viewport | content | cols | card | filter rows | overflow |
|---|---|---|---|---|---|
| 390 | 327 | 1 | 327 | 3 | **0** (was 193) |
| 768 | 689 | 2 | 336.5 | 2 | 0 |
| 960 | 545 | 2 | 264.5 | 2 | 0 |
| 1180 | 765 | 3 | 244.3 | 2 | 0 |
| 1280 | 865 | 3 | 277.7 | **1** (was 2) | 0 |
| 1366 | 951 | 3 | 306.3 | **1** (was 2) | 0 |
| 1440 | 1025 | **4** (was 3) | 244.3 | **1** (was 2) | 0 |
| 1920 | 1025 | **4** (was 3) | 244.3 | **1** (was 2) | 0 |
| 2560 | 1025 | **4** (was 3) | 244.3 | **1** (was 2) | 0 |

Two things the sweep settles beyond the fix itself. The content column tops out at 1025px and does
not grow past 1440 — the explorer is bounded by `--vp-layout-max-width`, not by the viewport, so
three columns is the ceiling on any monitor and the four-column question really is a card question.
And the filter bar reaches one row only at 1440 and above; at 1280 and 1366 it still takes two.

**Three items were deferred, then finished here, and they were all the same defect.** Four columns,
the phone overflow and the filter bar were each handed to C-103 as somebody else's problem. They were
not three problems. A flex or grid item's automatic minimum size is its **min-content**, so an
element refuses to be narrower than its longest unbreakable run and pushes its container instead:

| symptom | the run that set the floor | release |
|---|---|---|
| eight filters never shared a row, at any width | a `<select>`'s min-content is its **widest option** — "No operation-specific issue" is ~190px alone | `min-width: 0` + a flex basis |
| a phone scrolled sideways by 193px | a grid item held every row open at its longest request path | `min-width: 0` on `.row`, `overflow-wrap` on the path |
| the grid stuck at three columns however wide the page got | the card header's 274px min-content put a **314px floor** under a card | `flex-wrap: wrap` on `.card__head` |

That is why the earlier reasoning here — "four columns is a card change and not a grid change, so
320px is the smallest round number above the 314px floor" — was correct about the cause and wrong
about the conclusion. The floor was not a fact about the card; it was a fact about one missing
declaration. Once `.card__head` wraps, the floor is gone and the grid minimum re-tunes **320px →
240px** to spend the width it releases. The badge drops to its own line on a narrow card rather than
escaping the border.

**Recorded trade: the filter `<select>`s truncate.** At an 88px basis the long option strings clip —
"No operation-specific issue" is the worst of them. That is accepted, by coordinator decision, as the
price of a single-row bar: each control's own label sits directly above it, and the selected value is
short in the common case ("Any"). If the bar grows a ninth control this is the thing that breaks
first, and the honest fix then is shorter option text rather than a narrower basis.

**Fence.** Four columns, the `OperationRow` fix and the card header were C-103's by an earlier
scoping decision; the user lifted that fence and the coordinator confirmed the re-scope, so C-103
keeps only the density work genuinely left. Nothing here was taken without that.

## Notes
- Two routes, and the choice is the story's real content: **`layout: page`** in `explorer.md`
  frontmatter (drops sidebar and outline, gives full width, one line of diff), or a **scoped CSS
  override** of the doc layout's `max-width` under a page-specific class (keeps sidebar and outline,
  costs a rule that fights the theme). Pick one, record why.
- Verification here is a build plus looking at the page. Say that rather than inventing a unit test
  for CSS — `web/test/explorer.test.mjs` covers the pure selectors, and layout is not one.
