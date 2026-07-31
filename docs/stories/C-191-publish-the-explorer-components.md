---
id: C-191
title: "Publish the explorer components as an npm package — C-142's deferral condition is met"
pillar: Surfaces
status: ready
priority: 2
design: docs/designs/explorer-ux.md
epic: explorer-ux
areas: [web]
note: "C-142 deferred extraction 'until a second consumer exists'. babelforce's ai-agent-platform console is that second consumer, filed 2026-07-31. The deferral was conditional and the condition is now true"
---

# Publish the explorer components as an npm package

## Goal

Let a second Vue application mount this explorer without copying it, which is the only way the tier
boundary C-142 built survives contact with a second consumer.

## Why now — the deferral was conditional, and the condition is met

C-142 was explicit:

> **Do not extract an npm package.** `web/package.json` is `private: true` with no dependencies, and
> that stays true. Publishing a component library is a distributed artifact with its own versioning
> and consumers — a separate decision, deliberately deferred **until a second consumer exists**.

`~/babelforce/projects/ai-agent-platform`'s console is that second consumer: its
`web/packages/console/src/views/CapabilitiesView.vue` hand-renders operation detail that these
components already render better, and its F-81 decides which tiers it adopts. The alternative — it
copies the sources — recreates precisely the drift the tier boundary exists to prevent, in another
repository where this repository's test cannot see it.

## Acceptance

- [ ] A package publishes the components, `data/catalog.mts`'s types, and the `PathResolver` port
      (`PATH_RESOLVER`, `identityPath`). Vue is a **peer** dependency; nothing else is a runtime
      dependency.
- [ ] `no_component_imports_the_site_framework` still passes, and the packaging does not become a
      second way to import a component — the site consumes the same entry point an external host does,
      so the site is the package's first integration test.
- [ ] The hand-maintained-data guard still passes: no provider, service or address is named in any
      published source. Do not weaken it to make packaging easier.
- [ ] The three tiers (presentational / catalogue-aware / page) are the package's documented public
      surface, with the page tier marked as the one a host may reasonably decline — a host with its own
      router usually wants the first two.
- [ ] Versioning and release are written down, including what a breaking change to `catalog.mts`'s
      types means for a consumer.
- [ ] **Land [C-158](C-158-typescript-catalogue-types-drift.md) first.** `web/data/catalog.mts` is a
      third hand-enumeration of the catalogue shape with nothing in the gate holding it in step;
      publishing it as a consumer-facing type contract before that gate exists ships the drift.

## Notes

- Measured coupling, from C-142: fourteen components, six imported `vitepress`, between them exactly
  two symbols (`withBase` ×5, `inBrowser` ×1). All of that is already gone — this story is packaging,
  not decoupling.
- The consumer's own analysis (ai-agent-platform `docs/designs/connectors-host.md` §"The explorer")
  found the two catalogue models are cousins: `description`/`risk`/`idempotency` map, `parameters[]`
  vs `input_schema` do not, and the platform's datasource half has no counterpart here at all
  (C-137 is unbuilt). So the realistic consumer is the presentational and catalogue-aware tiers over
  connector-shaped data — worth knowing when deciding what the package's surface must be.
