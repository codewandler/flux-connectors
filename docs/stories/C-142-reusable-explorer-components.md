---
id: C-142
title: "Make the explorer components attachable, without extracting a package yet"
pillar: Codegen
status: ready
priority: 4
areas: [web]
note: "measured: the ENTIRE VitePress coupling is two functions — withBase in 5 components and inBrowser in 1 — and data/catalog.mts has zero imports. So this is a link port and a tier boundary, not a rewrite. The package boundary waits for a second consumer"
---

# Make the explorer components attachable, without extracting a package yet

## Goal

Let the explorer's Vue components render outside VitePress — in a flow editor, a console, or a
datasource view — without changing how they work inside it.

## What the coupling actually is, measured

Fourteen components. **Six** import from `vitepress`, and between them they use exactly two things:

| import | components | used for |
|---|---|---|
| `withBase` | 5 | prefixing an `href` |
| `inBrowser` | 1 | a `typeof window` guard |

No `useData`, no `useRouter`, no theme internals. And `web/data/catalog.mts` — the types and the pure
selectors — has **zero imports** of any kind, so the data contract is already portable.

The eight components with no VitePress import include the whole presentational tier: `SchemaBlock`,
`SpecChip`, `FluxSource`, `ParameterTable`, `StatusBadge`, `IssueNotice`, `CatalogExplorer`,
`ProviderCard`.

So this is a **link port and a stated tier boundary**, not a rewrite.

## Acceptance

- [ ] A **link port**: components take a path resolver — `(path: string) => string` — through
      `provide`/`inject` with an identity default, instead of importing `withBase`. VitePress
      provides `withBase`; anything else provides its own or nothing.
- [ ] `inBrowser` becomes a local `typeof window !== 'undefined'` guard.
- [ ] **No component reaches for data itself.** Every component takes what it renders as props or
      injected context. `OperationList` imports `data/catalog.mts` twice today; a component that
      fetches its own data cannot be attached anywhere.
- [ ] The tiers are stated somewhere a contributor will read — presentational (props only, no
      catalogue knowledge), catalogue-aware (typed against `data/catalog.mts`), page (owns routing and
      state). A new component says which tier it is.
- [ ] **The hand-maintained-data guard still passes.** The suite forbids naming a provider, service or
      address in explorer sources; a "reusable" component that hardcodes one would break the whole
      discipline, so this must not weaken it.
- [ ] **Failing-first test:** `no_component_imports_the_site_framework` — walk the component sources
      and assert none imports from `vitepress`. It must fail today, on six files.
- [ ] The site looks and behaves identically: `npm run build && npm test` green, and every existing
      assertion about rendered HTML unchanged.

## Notes

- **Do not extract a package in this story, and that is the main judgement in it.** `web/package.json`
  is `private: true` with no dependencies. Publishing a Vue component library is a distributed
  artifact with its own versioning, consumers and support burden — a different product from
  compiling connectors, and one `vision.md` neither blesses nor forbids. Deciding it before a second
  consumer exists is speculative. Do the cheap half now; the package boundary is a separate decision
  with a real consumer attached to it.
- **The cheap half is genuinely cheap** because of the measurement above: six files, two symbols.
  That is what makes doing it now defensible rather than premature.
- **Anticipate the datasource view** ([C-137](C-137-connectors-datasource-epic.md)) without building
  for it. `SchemaBlock` and `SpecChip` already render anything shaped like a schema or a tagged value,
  which is why they carry no coupling — that is the shape to copy, not a generic abstraction over
  record kinds nobody has needed yet.
- Keep the `FluxSource` precedent intact: it deliberately does **not** highlight, because shiki has no
  Flux grammar and colouring Flux by another language's rules would be worse than plain text. A
  reusable tier must not "improve" that.
