---
id: C-511
title: "Vendor datasources — a connector declares its data surface as a projection over its operations (epic)"
pillar: Spec
status: backlog
design: docs/designs/vendor-datasource-declarations.md
epic: vendor-datasources
areas: [connector-spec, catalog, migration]
note: "EPIC — Decision 0006 rules 5–6: vendor-data Datasource Definitions belong HERE, as a sixth member kind whose every read executes as an admitted operation. Seventeen of eighteen official plugins declare datasources through the protocol Milestone 5 deletes, and the IR has no replacement surface until this lands"
---

# Vendor datasources — a connector declares its data surface as a projection over its operations (epic)

## Goal

Give the connector IR a `[[datasources]]` surface per flux-roadmap Decision 0006 rule 6: a declared,
read-only projection over the connector's own operations, published into the manifest and both
catalogues, bound per tenant by Exchange and read by Flux only through the embedded Exchange client
— so the Milestone 4 migration waves have a mapped replacement for every plugin-declared datasource
before any deletion.

## Acceptance

- [ ] The `[[datasources]]` member exists in the IR: per-service namespace membership, entity
      schemas derived from the IR, per-verb operation bindings with cursor vocabulary, credential
      reach limited to the backing operation's declared auth, and load-time refusal of dangling
      bindings — [C-512](C-512-datasources-ir-member.md).
- [ ] The surface reaches the manifest, `catalog.json`/v1 and the embedded Rust catalogue from its
      first release, and never the generated `.flux` module —
      [C-513](C-513-publish-the-datasource-surface.md).
- [ ] `quirks.pagination` is superseded by the binding's cursor vocabulary and removed, not left
      declared-but-unreachable — [C-514](C-514-retire-quirks-pagination.md).
- [ ] [C-501](C-501-migrate-observability-plugins.md) and
      [C-502](C-502-migrate-data-and-secret-plugins.md) carry Decision 0006 rule 11's checkable
      no-deletion-without-mapped-replacement acceptance (amended in place by C-510 rather than
      duplicated here).
- [ ] Cursor/stream/lease terms are shared with
      [C-497](C-497-declare-runtime-operation-bindings.md)'s runtime-binding vocabulary — one
      spelling, cross-referenced, never re-invented.

## Children

- [C-512](C-512-datasources-ir-member.md) — the IR member: namespace, derived schema, bindings,
  `HashDomain`, validation
- [C-513](C-513-publish-the-datasource-surface.md) — manifest, public catalogue and embedded
  catalogue emission
- [C-514](C-514-retire-quirks-pagination.md) — retire `quirks.pagination` into the binding's cursor
  vocabulary

## Progress

- (not started — filed 2026-08-04 by C-510's adoption of Decision 0006)

## Notes

- Cross-repository source of truth:
  `../flux-roadmap/decisions/0006-datasources-are-declared-read-surfaces.md`. The design is
  [vendor-datasource-declarations.md](../designs/vendor-datasource-declarations.md).
- **This is not the catalogue datasource.** C-137…C-140 make the catalogue *about* connectors
  queryable, offline and in-process; this epic declares what a *vendor's* data surface knows, read
  through Exchange. The two must not share machinery beyond the published wire vocabulary Flux owns
  (Decision 0006 rule 4).
- **Sequencing:** nothing here precedes the Milestone 1 first-run path. The surface lands with the
  Milestone 2 runtime-declaration work (C-497); rule 11 makes it a hard predecessor of the
  Milestone 4 migration waves.
- **The pattern generalizes and the generalization is out of scope:** the write-capable board
  member (vendor status↔state mapping as a connector fact) is named by Decision 0006 and designed
  later, with Milestone 3 — vocabulary only, no story here builds it.
