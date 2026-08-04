---
id: C-510
title: "Adopt the Decision 0006 datasource vocabulary"
pillar: Bridge
status: done
priority: 1
design: docs/designs/vendor-datasource-declarations.md
epic: vendor-datasources
areas: [docs, connector-spec, migration]
note: "Decision 0006 defines the family's one datasource concept — a declared read-only record surface — and places vendor-data Datasource Definitions here. This reconciles C-137…C-140 onto the indexed DatasourceBackend they always needed and charters the [[datasources]] IR surface before Milestone 5 deletes the plugin channel"
---

# Adopt the Decision 0006 datasource vocabulary

## Goal

Make the cross-repository decision in `../flux-roadmap` authoritative here: a datasource is a named,
declared, read-only record surface; the catalogue datasource binds as an **indexed**
`DatasourceBackend`; and vendor-data Datasource Definitions become a chartered `[[datasources]]`
connector surface — reconciling this repository's designs and stories with Decision 0006 in one
atomic story, following the C-507 adoption pattern.

## Acceptance

- [x] [connectors-datasource.md](../designs/connectors-datasource.md) binds the catalogue as an
      indexed `DatasourceBackend` (search/get/list/relation/batch_get/sources over the compiled-in
      `catalog` crate), not `LiveDatasource`, and states its scope: the catalogue-*about*-connectors
      datasource, distinct from vendor-data datasource definitions. Offline, in-process, no
      socket/daemon constraints unchanged.
- [x] C-137…C-140 are amended before any dispatch (Decision 0006 rule 9): every
      `LiveDatasource`/`try_with_live_datasource`/two-op live projection reference is corrected to
      the indexed `DatasourceBackend` seam and its `try_register_datasource_ops` registration path;
      the acceptance op set is search/get/list/relation/batch_get/sources; C-139 additionally
      requires typed refusals from the trait's mutating methods on this read-only backend. Goals
      otherwise intact; each story notes the amendment.
- [x] [connector-surfaces.md](../designs/connector-surfaces.md) gains a `datasources` row: a
      *planned* surface with declared artifact reach M + R + J and never F, landing with the
      vendor-datasource design; `quirks.pagination`'s undecided cell is resolved to superseded and
      removed.
- [x] [vendor-datasource-declarations.md](../designs/vendor-datasource-declarations.md) (proposed)
      designs the `[[datasources]]` surface per rule 6: a projection over the connector's declared
      operations, per-service namespace membership, IR-derived entity schemas, per-verb operation
      bindings, credential reach as the backing operation's declared auth only, cursor vocabulary
      superseding `quirks.pagination`, the `HashDomain` answer, build-time enforcement, the board
      generalization named-not-designed, and explicit non-goals.
- [x] The `vendor-datasources` epic is filed with backlog children:
      [C-511](C-511-vendor-datasources-epic.md) (epic),
      [C-512](C-512-datasources-ir-member.md) (IR member),
      [C-513](C-513-publish-the-datasource-surface.md) (emission),
      [C-514](C-514-retire-quirks-pagination.md) (pagination supersession) — plus a roadmap
      narrative.
- [x] [C-501](C-501-migrate-observability-plugins.md) and
      [C-502](C-502-migrate-data-and-secret-plugins.md) carry rule 11's checkable
      no-deletion-without-mapped-replacement acceptance (amended in place — the rule belongs on the
      waves it gates, so no new story was cut for it), and
      [C-497](C-497-declare-runtime-operation-bindings.md) is cross-referenced as the owner of the
      cursor/stream/lease spelling.
- [x] The generated board, roadmap narratives and CHANGELOG entry are updated. This is a contract
      correction in the C-507 mould, so no failing-first behavioral test applies and no new runtime
      capability is claimed.

## Progress

- 2026-08-04: Adopted flux-roadmap Decision 0006. Verified the flux seam before amending anything:
  `flux_capabilities::DatasourceBackend` (`crates/flux-capabilities/src/datasource/mod.rs:108`,
  read in `/home/timo/projects/flux`) carries the six retrieval verbs plus mutating index methods,
  and registration is `try_register_datasource_ops` / `datasource_tools`
  (`.../datasource/ops.rs`); `LiveDatasource` is `schema`/`list`/`get` only, projected as two
  generated tools — confirming the decision's finding that C-137…C-140's acceptance could not be
  satisfied by the trait they named. Re-measured `quirks.pagination` in this tree: no reader
  outside the loader; declared by twilio (`providers/twilio.toml:306`, `:385`) and babelforce
  patches (`providers/babelforce.toml:600`, `:714`) — connector-surfaces.md's 2026-07-31 zendesk
  row is stale. Amended the two designs and six stories, filed the epic and its three children,
  and updated roadmap, CHANGELOG and board.

## Notes

- Cross-repository source of truth:
  `../flux-roadmap/decisions/0006-datasources-are-declared-read-surfaces.md`; the roadmap's
  "Datasources and boards" section carries the family narrative.
- The C-507 precedent applies throughout: accepted `../flux-roadmap` decisions take precedence over
  a conflicting sibling narrative, and adoption is one atomic story per repository.
- Implementation ordering is unchanged for Milestone 1 — nothing here touches the first-run path.
  C-137…C-140 stay `ready` at their existing priorities; the vendor surface lands with Milestone 2's
  C-497 work.
