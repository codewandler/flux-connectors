---
id: C-238
title: "Two explorers render the same catalogue in two codebases, and only one of them is maintained"
pillar: Surfaces
status: blocked
priority: 2
design: docs/designs/host-explorer.md
epic: host-explorer
areas: [host, web]
note: "phase 2 — BLOCKED on C-158 then C-191. C-142 already detached the components from VitePress for exactly this, and their README marks the page tier as 'the one a host may reasonably decline'"
---

# The host mounts the explorer components rather than restating them

## Goal

Give the operator's page the site's visual language and information design by **mounting the same
components**, so one improvement lands in both places and neither drifts.

## Why this is reuse and not a rewrite

[C-142](C-142-reusable-explorer-components.md) did the hard part before anyone asked.
`web/.vitepress/theme/components/README.md`:

> Since C-142 **none of them imports VitePress**, so the set can be mounted somewhere other than this
> site — a product's own admin surface, a Storybook, a test harness — without a rewrite.

A component may import Vue, a sibling, and `data/catalog.mts`; the one framework dependency is the
`PATH_RESOLVER` port with an identity default. The three tiers are already documented, with the page
tier marked *"the one a host may reasonably decline"* — which is what this host does, because it owns
its own routing and its own operational state.

## The seam, measured

Neither shape is a subset of the other. Verified by reading the components:

| tier | components | verdict |
|---|---|---|
| Presentational | `SchemaBlock`, `SpecChip`, `FluxSource` | **reusable as-is** — plain values |
| Catalogue-aware | `StatusBadge` (`:13` calls `ownIssues`), `ProviderCard` (`:43` reads `operations[].status.works`), `ParameterTable`, `IssueNotice` | need the **catalogue** shape |
| Page | `CatalogExplorer`, `OperationList`, `OperationDetail`, `CoreDetail` | **declined** |

The host has `wiring`, `callable`, `stored`, `settings`; the catalogue has `status{works, issues}`,
`parameters`, schemas.

**The host cannot synthesise the catalogue shape, and this is sharper than the design first stated.**
`catalog::Operation` (`crates/catalog/src/lib.rs:138`) carries exactly `id, provider, service,
description, risk, idempotency, credentials, hosts, flux` — **no `method`, no `path`, no
`parameters`, no schemas, no `status`** — and `catalog::Provider` has no `services`. So the
components need fields the embedded catalogue does not hold, and the obstacle is not only that the
emitter lives in a compiler crate. `include_bytes!` of the published document is the route.

Growing `crates/catalog` to carry them is **rejected**: it is in the publish closure, is documented
as dependency-free `.rodata`, and this would multiply its size for consumers that only want to
execute.

## Acceptance

- [ ] **Failing-first test:** the host serves catalogue-shaped JSON that the components render
      unchanged. Name it.
- [ ] The host serves the catalogue **alongside** its operational JSON, and the operational overlay
      (wiring, stored, callable, run) is rendered by host-owned components. Teaching the shared
      components a second port for operational state is **refused**: `stored` is a fact about a
      credential the public site is forbidden to know about.
- [ ] **No second emitter of `catalog.json`.** Take route (a) from the design — embed the committed
      document — and record route (b) as the follow-on. A second emitter of one document is the drift
      this repository exists to prevent, and taking it quietly is the failure mode.
- [ ] The build output is **committed and served via `include_str!` per asset**, not `ServeDir` — a
      filesystem read would be the first in a binary whose current property is that the page is
      compiled in.
- [ ] A staleness test rebuilds in CI and asserts the committed output is byte-identical, the same
      shape as `connector-cli -- diff` for the catalogue. **Plus an offline Rust check**, so
      `cargo test --workspace` stays self-sufficient: the build writes a `SOURCES.sha256` of every
      input, and a Rust test recomputes it naming the file that changed. `sha2` is **already** a
      dependency of `connectors-api`, so this needs no fenced-manifest edit.
- [ ] **A test that the embedded catalogue and the embedded JSON agree** — equal provider-id sets and
      equal per-provider operation-id sets, naming the first divergence and pointing at
      `cargo run -p connector-cli -- build`. Two generated views of one IR now sit in one binary; this
      is the check that keeps them honest, and it is stronger than anything that exists today.
- [ ] **The new `/assets/*` routes are added to BOTH secret sweeps.** `tests/host.rs:101` and
      `tests/dev_signin.rs:502` each enumerate the paths they check for the three sentinels; a new
      served surface neither list covers is a hole in the guarantee, not a new file.
- [ ] **A guard that the stylesheet defines every token the components use.** 25 distinct `--vp-*`
      custom properties are referenced across the components; that set *is* the styling adapter.
      Without the guard, a component upgrade renders a transparent element and nothing fails.
- [ ] **No external or CDN asset.** The host's defence is that it has none, and a fetch from a CDN
      would be its first external dependency.
- [ ] `no_component_imports_the_site_framework` still passes, and the packaging does not become a
      second way to import a component.
- [ ] The hand-maintained-data guard still passes: no provider, service, host, credential, operation
      id or issue code named in any component source.
- [ ] Everything in the design's §"Constraints any implementation must hold" survives, and all three
      sign-in states stay reachable.
- [ ] Both gates green, the web gate in its documented order.

## Notes

- **Blocked, in order:** [C-158](C-158-typescript-catalogue-types-drift.md)
  then [C-191](C-191-publish-the-explorer-components.md). C-191's other blocker, C-205, is done, and
  this story makes the console C-191's **second consumer** — the integration proof its acceptance
  asks for and cannot get from the site alone.
- **The build root goes at `crates/connectors-api/ui/`, not under `web/`.** A credential-collecting
  console living under the public site's tree would make *"the site never collects a credential"* a
  fact about build configuration rather than about location. Keeping it with the host keeps the
  C-147 boundary legible to a reviewer. The cost is honest: `AGENTS.md`'s gate section gains a second
  Node command.
- **No `build.rs`.** A build script shelling out to npm would put a Node toolchain and a network
  fetch inside `cargo build --workspace`.
- Two open stories land acceptance in the file this story replaces —
  [C-225](C-225-a-config-field-cannot-declare-a-closed-set-of-values.md) and
  [C-226](C-226-one-credential-cannot-be-shared-by-two-connectors.md) both name `index.html` by path.
  Either land them first, or re-point those lines at a named component. Do not let an implementor
  discover it mid-rewrite.
- **A future CSP gets easier, not harder.** There is no Content-Security-Policy anywhere today, which
  is the only reason the current inline `<style>`/`<script>` works at all. External bundled files
  remove that obstacle; going further inline would entrench it. Say so where the decision is recorded
  so it reads as a gain rather than an accident.
- Do not diverge from [C-99](C-99-explorer-ux-epic.md). Density and filtering are shared thinking.
- **Small correction while in here:** `web/.vitepress/theme/components/README.md` opens "These
  fourteen Vue components" and there are **fifteen** — `InboundSurface.vue` is absent from its tier
  tables. Same class as [C-81](C-81-declared-counts-are-checked.md); fix it rather than filing it.
