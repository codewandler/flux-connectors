# Design: the explorer at fleet scale

**Status:** proposed · **Pillar:** Surfaces · **Epic:** `explorer-ux` · **Stories:** C-99 … C-103

## Why

The explorer was designed against **6 providers and 25 operations** (C-42–C-45). It now indexes
**16 providers, 18 services and 88 operations**, and the seams show — not as bugs, but as decisions
that were right at a third of the size and are wrong now.

The comments in the components say so themselves. `OperationList.vue` still reasons about *"all 25
operations"* and *"the five with a problem of their own"*; `CatalogExplorer.vue` reasons about *"N of
25 working"*. Those numbers are stale, which [C-81](../stories/C-81-declared-counts-are-checked.md)
already owns as a class of defect. What this epic owns is the layout and interaction those numbers
were sized for.

## The width problem is one measured fact, not a matter of taste

`web/explorer.md` carries `outline: [2, 2]`, so it renders in VitePress's **doc** layout. That layout
caps the content column at a fixed width:

```
node_modules/vitepress/dist/client/theme-default/components/VPDoc.vue:191
  max-width: 688px;

node_modules/vitepress/dist/client/theme-default/styles/vars.css:313
  --vp-layout-max-width: 1440px;
```

So the explorer renders 88 operations and 16 provider cards inside **688px**, on a page that is
allowed to be 1440px. Two consequences follow arithmetically:

- `CatalogExplorer.vue`'s provider grid is `repeat(auto-fit, minmax(320px, 1fr))`. At 688px that is
  **exactly two columns**, so 16 connectors take eight rows of scrolling.
- The five-control filter bar in `OperationList.vue` is a `flex-wrap` row. At 688px it wraps to two or
  three lines, so the controls move as the viewport changes.

The fix is a layout choice, not a stylesheet patch — see C-100.

## What the explorer cannot see

**Services are invisible.** [C-49](../stories/C-49-provider-services.md) made a service the middle
addressing level — the unit you address, version, select and install — and the catalogue publishes 18
of them with their own `base_url`, `api_version`, `gid` and operation count. The explorer has no
service filter, and `ProviderCard.vue` does not mention services at all. Google ships three and reads
as one.

**Filter state is not shareable.** `explorer.md` promises *"Every operation has a stable page you can
share directly"* — true of an operation, false of a *view*. Filtering is component state, so "every
destructive Shopify operation" cannot be sent to anyone.

## What the explorer must not show yet, and why that is not this epic's problem

Four waves added events, channel bindings, configuration fields and flow graphs to the IR. **None of
it reaches `catalog.json`** — that is [C-83](../stories/C-83-channel-binding-codegen.md) and
[C-87](../stories/C-87-configuration-codegen.md), unstarted. A story here that tried to render an
inbound surface would block on codegen and stall the wave.

So this epic is scoped to what the published document already carries. When C-83/C-87 land, the
explorer gains a genuinely new dimension and that is its own story.

## The editorial decision that must survive a redesign

This is the part most at risk from a pass aimed at making things prettier, and it is written down
here so that nobody removes it by accident.

The explorer deliberately does **not** say *"N of 88 operations working"*. `works` is false for every
operation, because no provider can make a live call until the `$auth` seam lands in flux. A headline
reading "0 of 88 work" would be accurate and would misrepresent the project, since roughly eighty of
those operations are exactly as designed and waiting on one shared seam.

So presentation follows the `scope` the emitter puts on every issue — `catalog` renders once as a
banner, `provider` on that connector's card, `operation` as a badge on the operation. `CatalogExplorer.vue`
records the reasoning inline, and `OperationList.vue`'s last filter is deliberately *"has a limitation
of its own"* rather than *"does it work"*, because filtering on `works` sorts nothing from nothing.

**Any redesign preserves that distinction or it is a regression**, however much better it looks.

## Approach

Four stories, ordered so each is independently shippable:

1. **C-100 — width.** Move the explorer to a full-width layout. Largest visible gain, smallest diff,
   no data change.
2. **C-101 — services.** A service filter and service facts on the provider card. Renders a dimension
   the catalogue already publishes and nothing surfaces.
3. **C-102 — shareable views.** Filter state in the query string, plus a sort. Makes the page's own
   shareability promise true of a view and not only of an operation.
4. **C-103 — density.** The card and row treatment a 16×88 catalogue needs: scan-ability, grouping,
   and the honest-status presentation above, restated in whatever the new layout is.

## Testing

`web/test/explorer.test.mjs` (361 lines, `node --test`) covers the pure selectors in
`web/data/catalog.mts`. That is the seam to keep using: **filter and sort logic belongs in
`catalog.mts` as pure functions**, so a behavioural change ships with a failing-first test there
rather than as an untested component method. Layout and styling are verified by build plus a look at
the rendered page, and the stories say so plainly rather than pretending a unit test covers CSS.

## Alternatives considered

- **A bespoke app instead of VitePress.** Rejected: the site's own theme entry states the intent —
  *"documentation with an interactive index over generated data, not a bespoke app"* — and the
  docs/explorer split is the reason the catalogue is browsable at all.
- **Widening every page.** The doc layout is right for prose. Only the explorer has a wide table and a
  card grid; widening the overview would make its paragraphs harder to read, not easier.
- **Client-side pagination of the operation list.** 88 rows render fine, and the current list works
  with JavaScript disabled (`OperationList.vue` filters a fully-rendered list). Paginating would trade
  a real property for a problem the catalogue does not yet have. Revisit past a few hundred.
