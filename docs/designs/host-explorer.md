# The host's explorer — an operator console that converges on the site's components

**Status:** proposed · **Epic:** `host-explorer` · **Stories:** C-236 … C-239

## Why this exists

The owner ran the app on 2026-07-31 and reported it working but visibly weaker than the public
documentation site's explorer. That comparison is fair, and the reason is worth recording before
anyone treats it as neglect.

**No story ever asked for more.** [C-203](../stories/C-203-connectors-api-skeleton.md) scoped the
host as *"the vertical slice: paste a token, pick an operation, get a real vendor response. No
sign-in, no OAuth, **no UI beyond what proves it**"*, and every change to
`crates/connectors-api/src/index.html` since has been a rider on a backend story —
[C-212](../stories/C-212-the-host-repeats-the-connected-conflation.md)'s third wiring state,
[C-234](../stories/C-234-a-dev-sign-in-that-needs-no-google-registration.md)'s dev button. There is
**no written decision to keep it minimal** and no prohibition on assets or a build step. Meanwhile
[explorer-ux.md](explorer-ux.md) — the only document in this repository that reasons about explorer
layout, density, filters and shareable views — is scoped exclusively to `web/` and never mentions
this page.

The measured gap: **355 lines** (`include_str!`, no build step, no assets, no favicon) against
**2,434 lines** of Vue across 15 components plus 771 lines of pure selectors in
`web/data/catalog.mts`.

It is thin, not sloppy. It renders C-212's four wiring states, per-operation callability and an
OR-of-AND credential requirement; it is deliberately XSS-safe (`textContent`, never `innerHTML`) and
CSRF-aware, each with its reasoning inline.

## The asymmetry that shapes every decision

`crates/connectors-api/src/ui.rs` states it, and it is not incidental:

> `web/` is a public GitHub Pages site, and [C-147](../stories/C-147-explorer-runs-an-operation.md)'s
> acceptance forbids it collecting a credential or implying a live call. This surface is the exact
> opposite on both counts: it does call the vendor, it must say "sent", and collecting a credential
> is the point. Its safety comes from being loopback-only and unpublished, which is a property the
> public site structurally cannot have, and vice versa.

**So credential capture and execution can never move into shared components.** Convergence means
sharing the *browsing* half and keeping the *operating* half here. Any proposal that puts a token
field in a component `web/` also mounts is refused by C-147, not by taste.

## What is already true and should be reused, not rebuilt

[C-142](../stories/C-142-reusable-explorer-components.md) detached the components from VitePress
before anyone asked for this. `web/.vitepress/theme/components/README.md`:

> Since C-142 **none of them imports VitePress**, so the set can be mounted somewhere other than
> this site — a product's own admin surface, a Storybook, a test harness — without a rewrite.

A component may import Vue, a sibling component, and `data/catalog.mts`. Nothing else, enforced by
`no_component_imports_the_site_framework`. The single framework dependency is a **port** with an
identity default (`PATH_RESOLVER`), and the three tiers are documented with the page tier marked as
*"the one a host may reasonably decline"* — which is precisely what this host should do.

## The seam, which is the hard part

The two surfaces consume **different shapes, and neither is a subset of the other**:

| | browsing facts | operational facts |
|---|---|---|
| `catalog.json` (site) | `Operation.status{works, issues, notes}`, `parameters`, schemas, `flux` | — |
| host `/v1` API | `id`, `description`, `risk`, `service`, `hosts` | `wiring`, `callable_operations`, `operations[].callable`, `credentials[].stored`, `settings` |

Measured coupling, by reading the components:

- **Reusable as-is** — `SchemaBlock.vue`, `SpecChip.vue`, `FluxSource.vue` take plain values.
- **Needs the catalogue shape** — `StatusBadge.vue:13` calls `ownIssues(props.operation)`;
  `ProviderCard.vue:43` reads `provider.operations[].status.works`.
- **Declined** — the page tier (`CatalogExplorer`, `OperationList`, `OperationDetail`, `CoreDetail`),
  because this host owns its own routing and its own operational state.

**Decision: the host serves catalogue-shaped JSON alongside its operational JSON.** Components render
the catalogue unchanged; a thin host-owned layer renders the operational overlay. The alternative —
teaching the components a second port for operational state — doubles the port surface and couples
`web/` to a shape it must never render, since `stored` is a fact about a credential the site is
forbidden to know about.

**The obstacle, named up front.** `catalog.json`'s shape is emitted by
`crates/connector-cli/src/site.rs`, a **compiler crate**. `connectors-api` depends on `catalog`, not
on `connector-cli`, and linking the compiler into the host would be wrong even though
`crates/connector-cli/tests/dependency_fence.rs` is directional and would not catch it. Two routes:

- **(a) Embed the committed `web/public/catalog.json`.** Zero drift by construction — literally the
  same bytes the site renders. Costs ~1.8 MB in the binary and a build-time path from a crate into
  `web/`.
- **(b) Move the emitter into a crate both can use.** No duplicate emitter, but a larger refactor
  that touches the publish closure.

**Start with (a), record (b) as the follow-on.** (a) is reversible; a second emitter of the same
document would be exactly the drift this repository exists to prevent, and is the one option that
must not be taken quietly.

## The build step, and what it buys beyond looks

`web/` already builds with one dependency (`vitepress`) on Node 22+.
[C-191](../stories/C-191-publish-the-explorer-components.md) already specifies the package: Vue as a
**peer** dependency, nothing else at runtime, the three tiers as its public surface.

Output is **committed and served via `include_str!` per asset**, not `ServeDir` — a filesystem read
would be the first in a binary whose current property is that the page is compiled in. A staleness
test must rebuild in CI and assert byte-identical output, the same shape as `connector-cli -- diff`
for the catalogue.

Two gains that are not cosmetic:

- **It closes a recorded test debt.** There is no JS harness, and C-234 could not close its mutation
  M15 (`index.html`'s `if (status.dev)` guard) for that reason, while `AGENTS.md` requires a
  failing-first test for a behavioural change. `web/test/explorer.test.mjs` asserts against built
  HTML *and* the emitted stylesheet — layout regressions caught without screenshots. Copy that.
- **It makes a future CSP possible.** There is no Content-Security-Policy anywhere today, and that
  is the only reason the inline `<style>`/`<script>` works. External bundled files make a CSP
  *easier*; going further inline makes it harder.

## Constraints any implementation must hold

- `textContent`, never `innerHTML`. Auth state changes by `fetch` POST, never a link — `SameSite=Lax`
  lets a cross-site GET carry the cookie.
- The dev button drawn **only** when `status.dev`, labelled so it cannot be mistaken for real
  sign-in (C-234).
- The `wiring` tokens stay character-identical to C-206's catalogue tokens. Restating them in
  different words is how two surfaces describing one fact come to disagree.
- All three sign-in states reachable: unconfigured → setup instructions; signed out → doors; signed
  in → catalogue.
- **No external or CDN assets.** The host's defence is that it has none.
- `tests/host.rs::a_stored_credential_reaches_no_surface` keeps passing: `/` returns 2xx and carries
  none of the three sentinels.
- No component hardcodes a provider, vendor, service, host, credential, operation id or issue code.

## Deliberately out of scope

- Anything that would let `web/` collect a credential or imply a live call. C-147 is not negotiable.
- The site's own fleet-scale work — [C-99](../stories/C-99-explorer-ux-epic.md) and its children own
  width, services, shareable views and density for `web/`. This epic must not diverge from them; the
  filter and density thinking is shared, and duplicating it is the failure mode.
