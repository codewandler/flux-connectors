---
id: C-102
title: Make a filtered view shareable, and let the list be sorted
pillar: Surfaces
status: in-progress
priority: 4
design: docs/designs/explorer-ux.md
epic: explorer-ux
areas: [web]
note: "the page promises 'every operation has a stable page you can share' — true of an operation, false of a view. 'Every destructive Shopify operation' cannot be sent to anyone"
---

# Make a filtered view shareable, and let the list be sorted

## Goal
Make the explorer's own shareability promise true of a *view*, not only of an operation.

## Acceptance
- [x] Filter state lives in the query string — connector, service, risk, idempotency, defect and the
      search term. Opening the URL restores the view exactly.
- [x] Changing a filter **replaces** rather than pushes, so the back button leaves the explorer instead
      of stepping back through every keystroke of a search.
- [x] An empty filter contributes no parameter, so the unfiltered URL is clean and two routes to the
      same view produce the same string.
- [x] Unknown or stale parameters are **ignored, not fatal** — a shared link outliving a renamed
      connector degrades to a wider view rather than an error.
- [x] The operation list can be sorted: catalogue order (the default, and it is meaningful — it is the
      order the module emits), id, and risk. Sort state is in the URL like the filters.
- [x] Failing-first test in `web/test/explorer.test.mjs`: **serialise → parse round-trips** for a
      representative set of states, including empty and unknown-parameter cases. The
      encode/decode pair is pure and belongs in `web/data/catalog.mts`.
- [x] Works with JavaScript disabled to the same degree as today — the current list renders every
      operation server-side and filters client-side, and that property is not traded away.

## Progress
- **Done.** `web/data/catalog.mts` gained the view section: `View`, `emptyView`, `encodeView`,
  `decodeView`, `narrowView`, `compareOperations`, `sortOperations`, plus the declared `SORTS` and
  `RISK_ORDER`. `OperationList.vue` holds one `view` ref instead of six, reads the URL in
  `onMounted` and writes it back through `history.replaceState`.
- *Ignored, not fatal* is two halves, deliberately. `decodeView` is pure and drops unknown parameter
  names and unrecognised values of the vocabularies this site owns (sort, defect). It cannot judge a
  connector or a service, because a pure parse has no catalogue — `narrowView` does that against the
  providers and drops anything no longer offered, so a stale link widens rather than empties.
- `narrowView` also absorbed the dependent service narrowing that C-101 left as a `watch(services)`
  in the component, so the one rule now has one home.
- Verified in a real browser as well as in the suite (headless Chrome over CDP, throwaway script):
  three filter changes left `history.length` at 3; `?connector=nonesuch&service=nonesuch&sort=nonesuch&nonesuch=1&risk=destructive`
  settles at `?risk=destructive` with the page intact; 88 rows render before hydration, in catalogue
  order.

## Notes
- VitePress runs on Vue Router; use its `replace` rather than touching `history` directly, so the
  SPA navigation and the router's state stay in agreement.
  - **This premise did not hold.** VitePress 1.6.4 ships its own router and `vue-router` is not a
    dependency of `web/` at all; the router it does ship exposes only `go`, which pushes. Adding
    `vue-router` would be a new dependency for a history call. So the wiring uses
    `history.replaceState`, passing `history.state` through untouched so the router's own scroll
    state survives — which is what the note was protecting.
- Sorting by risk needs a declared order (`low < medium < high < destructive`) rather than alphabetical
  — `destructive` sorting between `default` and `high` would be silently wrong.
  - Declared as `RISK_ORDER`, and the test asserts both the order itself and that every tier the
    catalogue publishes is ranked by it, so a new tier fails rather than sorting last by accident.
