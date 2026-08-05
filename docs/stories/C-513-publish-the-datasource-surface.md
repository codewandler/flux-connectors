---
id: C-513
title: "Publish the datasource surface into the manifest, the public catalogue and the embedded catalogue"
pillar: Codegen
status: backlog
design: docs/designs/vendor-datasource-declarations.md
epic: vendor-datasources
areas: [connector-cli, catalog, web]
note: "Decision 0006 rule 6 makes non-empty artifact reach an ENTRY criterion: [[datasources]] reaches M + catalog.json + the embedded Rust catalogue from its first release, and never the generated .flux module — the plugin-manifest declared-then-dropped failure must not recur"
---

# Publish the datasource surface into the manifest, the public catalogue and the embedded catalogue

## Goal

Make a declared datasource member visible to every consumer that acts on it — Exchange binding a
tenant Datasource, Flux validating a registration, an operator or explorer reading the catalogue —
so the surface ships observable rather than joining the dead-surface table in
[connector-surfaces.md](../designs/connector-surfaces.md).

## Acceptance

- [ ] A `[[datasources]]` member is emitted into the service manifest
      (`connectors/<provider>[-<service>].connector.toml`), respecting service selection — a
      manifest never carries a member of a service it does not own.
- [ ] It is emitted into `web/public/catalog.json`, with the binding, entity and cursor facts a
      consumer needs to bind it — and no credential material of any kind. (`web/public/v1/**` is
      the flux-core catalogue — [C-112](C-112-publish-flux-core-specifications-in-the-explorer.md)
      — not a connector artifact, and out of this surface's reach.)
- [ ] It is emitted into the embedded Rust catalogue (`crates/catalog/src/generated/<provider>.rs`).
- [ ] **Nothing reaches the generated `.flux` module.** The emitter refuses to dress a datasource
      up as an `op`, exactly as it refuses a pollable event — flux lifts `op` declarations only,
      and the read seam is Exchange's.
- [ ] **Failing-first test:** a fixture connector declaring a datasource round-trips provider
      source → IR → manifest + both catalogues, and each artifact carries the member; a second
      assertion holds the module byte-free of it.
- [ ] `flux-connectors diff` stays a fixed point; whole-catalogue artifacts follow the
      coordinator-owned regeneration rule.
- [ ] The gate is green.

## Progress

- (not started)

## Notes

- Depends on [C-512](C-512-datasources-ir-member.md) for the member; the two ship in one release so
  the surface never releases IR-only (Decision 0006 rules 6 and 12 — declared surfaces are
  enforced, not decorative).
- Design: [vendor-datasource-declarations.md](../designs/vendor-datasource-declarations.md). The
  artifact-key vocabulary (F/M/R/J) is connector-surfaces.md's.
- `catalog.json` schema changes are versioned and breaking for public catalogue consumers — see
  C-87's precedent in the CHANGELOG for how that is recorded.
