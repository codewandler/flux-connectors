---
id: C-142
title: "Detach the explorer components from VitePress: a link port and a tier boundary"
pillar: Codegen
status: done
areas: [web]
note: "measured, not guessed: 6 of 14 components import `vitepress`, and between them they use exactly two symbols. No `useData`, no `useRouter`, no theme internals — so this is a link port and a tier boundary, not a rewrite"
---

# Detach the explorer components from VitePress: a link port and a tier boundary

## Goal

Make the explorer's Vue components attachable somewhere other than this VitePress site, without
extracting a package and without changing a single byte of what the site renders.

The measurement this rests on was already taken. Fourteen components; **six** import from
`vitepress`, and between them they use exactly **two** symbols:

| import | components | used for |
|---|---|---|
| `withBase` | 5 | prefixing an `href` |
| `inBrowser` | 1 | a `typeof window` guard |

No `useData`, no `useRouter`, no theme internals. `web/data/catalog.mts` has **zero** imports, so the
data contract is already portable. Eight components — `SchemaBlock`, `SpecChip`, `FluxSource`,
`ParameterTable`, `StatusBadge`, `IssueNotice`, `CatalogExplorer`, `ProviderCard` — have no coupling
at all.

So the work is a **link port and a tier boundary, not a rewrite**.

## Acceptance

- [x] **A link port.** Components take a path resolver `(path: string) => string` through
      `provide`/`inject` with an **identity default**, instead of importing `withBase`. VitePress
      supplies `withBase`; anything else supplies its own or nothing.
- [x] **`inBrowser` becomes a local `typeof window !== 'undefined'` guard.**
- [x] **No component reaches for data itself.** Everything it renders arrives as props or injected
      context — a component that fetches cannot be attached anywhere.
- [x] **The tiers are written down where a contributor will read them:** presentational (props only,
      no catalogue knowledge) · catalogue-aware (typed against `data/catalog.mts`) · page (owns
      routing and state).
- [x] **Failing-first test:** `no_component_imports_the_site_framework` — walk the component sources
      and assert none imports from `vitepress`. It must fail today, naming **six** files.
- [x] **The site behaves identically.** Every existing assertion about rendered HTML unchanged.

## Constraints

- **The hand-maintained-data guard must still pass.** The suite forbids naming a provider, service or
  address in explorer sources. A "reusable" component that hardcoded one would break the discipline
  the whole catalogue depends on. Do not weaken that test to make an extraction easier.
- **Do not extract an npm package.** `web/package.json` is `private: true` with no dependencies, and
  that stays true. Publishing a component library is a distributed artifact with its own versioning
  and consumers — a separate decision, deliberately deferred until a second consumer exists.
- **Keep the `FluxSource` precedent intact.** It deliberately does *not* highlight, because shiki has
  no Flux grammar and colouring Flux by another language's rules would be worse than plain text.

## Progress

Done on `impl/C-142`. The gate is `cd web && npm run build && npm test`: **24 passing before, 25
after**, and the new test failed at the merge base naming exactly the six coupled files.

**The port.** `PathResolver`, `PATH_RESOLVER` and `identityPath` live in `web/data/catalog.mts`,
beside `operationHref`/`coreEntryHref` — those answer *which page*, the resolver answers *how a host
spells an href*. The key is a plain string rather than a Vue `InjectionKey` on purpose: `catalog.mts`
still has **zero imports**, which is what keeps it readable both by a component and by the
build-time `*.paths.mts` loaders under plain Node.

`web/.vitepress/theme/index.mts` is now the **only** file under `theme/` that names the framework —
it does `app.provide(PATH_RESOLVER, withBase)` in `enhanceApp`. `withBase` is a plain function over
VitePress's site-data module, not a composable, so it carries across the provide intact including
through the server render.

**Behaviour.** Beyond the suite, the merge base's `dist` and the branch's were built and compared
page by page: identical for every page once Vue's scoped-style ids and the chunk hashes (both of
which move with the build root) are normalised away. Nothing rendered changed.

**Tiers**, recorded in `web/.vitepress/theme/components/README.md`, with a pointer from
`web/README.md`'s layout table:

- *presentational* — `FluxSource`, `SchemaBlock`, `SpecChip`
- *catalogue-aware* — `IssueNotice`, `ParameterTable`, `StatusBadge`, `ProviderCard`,
  `OperationRow`, `CoreExplorer`, `CatalogSnapshot`
- *page* — `CatalogExplorer`, `OperationDetail`, `CoreDetail`, `OperationList`

The test enforces the boundary as an allow-list, not just a `vitepress` ban: a component may import
Vue, a sibling component, or `data/catalog.mts`, and nothing else — so `node:*` and the build-time
loader `data/catalog.data.mts` are excluded by construction.

No package was extracted; `web/package.json` is untouched and still `private: true` with no
dependencies.
