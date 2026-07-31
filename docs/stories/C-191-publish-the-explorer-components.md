---
id: C-191
title: "Publish the explorer components as an npm package — C-142's deferral condition is met"
pillar: Surfaces
status: blocked
priority: 2
design: docs/designs/explorer-ux.md
epic: explorer-ux
areas: [web]
note: "BLOCKED on C-158 only. C-205 is DONE (the web gate is green -- verified 32/32 on 2026-07-31, and the earlier red reading was npm test run without the npm run build that precedes it). C-142 deferred extraction until a second consumer exists; babelforce's console is one, and C-238 makes this host a second"
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

## Progress

- **2026-07-31 — dispatched, and parked without a diff.** Two independent blockers, both traced to
  the merge base by the implementor and both re-verified by the coordinator against a clean `main`.
  No branch was created and no file was changed, so a re-dispatch costs nothing.

  1. **This story's own Acceptance requires C-158 first** — *"Land C-158 first"*, on the recorded
     reasoning that publishing `catalog.mts` as a consumer-facing type contract before that gate
     exists "ships the drift". [C-158](C-158-typescript-catalogue-types-drift.md) is `ready` and
     unimplemented; the only commit naming it is the one that filed it.

  2. **The web gate is red on `main`**, in the exact guard Acceptance item 3 depends on.
     `cd web && npm ci && npm run build && npm test` reports **27 pass, 1 fail** —
     `nothing about the catalogue is hand-maintained in the explorer sources`. Verified by the
     coordinator independently of the implementor's report. It is a false positive: Postmark
     declares a service named `server`, the guard forbids every service name as a raw substring,
     and `web/data/catalog.data.mts:20` uses the word in a comment about the dev server. Filed as
     [C-205](C-205-service-name-guard-matches-english-prose.md), which also records that **12 more
     service names are ordinary English words**.

  Confirmed already-done while investigating, so the remaining work really is packaging only:
  `PathResolver`, `PATH_RESOLVER` and `identityPath` exist at `web/data/catalog.mts:385-391`,
  `no_component_imports_the_site_framework` passes, and the three-tier public surface Acceptance
  item 4 asks to document is already written up in
  `web/.vitepress/theme/components/README.md`.

  **Dispatch order to unblock: C-205 → C-158 → C-191.** All three write
  `web/test/explorer.test.mjs`, so they take one wave slot between them, never three.
