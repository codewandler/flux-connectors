---
id: C-139
title: "The indexed DatasourceBackend over the catalogue, and its registration"
pillar: Bridge
status: ready
priority: 4
design: docs/designs/connectors-datasource.md
epic: connectors-datasource
areas: [bridge]
note: "implements flux's indexed DatasourceBackend trait over the compiled-in catalogue (Decision 0006 rule 9 — not LiveDatasource). Registers its six retrieval ops into the SAME ToolRegistry as the Tool pack; the trait's mutating methods return typed refusals on this read-only backend"
---

# The indexed DatasourceBackend over the catalogue, and its registration

## Goal

Implement `flux_capabilities::DatasourceBackend` over the connectors catalogue, so a host can
register the six-op retrieval pack with `try_register_datasource_ops` and query the catalogue from
a session.

## Acceptance

- [ ] A backend implementing `DatasourceBackend`
      (`crates/flux-capabilities/src/datasource/mod.rs:108`, read 2026-08-04), reading the
      **compiled-in `catalog` crate** — not `catalog.json` from disk. Offline and deterministic by
      construction; a runtime file path can be missing, stale or edited, for no benefit a rebuild
      does not already give.
- [ ] It registers through `try_register_datasource_ops(registry, backend)`
      (`crates/flux-capabilities/src/datasource/ops.rs`), installing the six retrieval ops —
      **search / get / list / relation / batch_get / sources** — into the same `ToolRegistry` a
      host hands the Tool pack's declarations.
- [ ] `get`, `list`, `relation`, `batch_get` and `sources` are answered from the entity model of
      [C-138](C-138-datasource-entity-model.md). Search is [C-140](C-140-datasource-search.md) and may
      be a naive placeholder here — but say so, and do not let a placeholder ship as if it were the
      feature.
- [ ] **The mutating trait methods return typed refusals.** `upsert`, `clear`, `delete_source` and
      `delete` refuse on this read-only backend — a typed error naming the reason, never a silent
      no-op that reports success for a write that did not happen. **Failing-first test:**
      `every_mutating_method_refuses_and_names_the_backend_read_only`.
- [ ] **Failing-first test:** `the_datasource_registers_beside_a_tool_pack_without_collision` — build
      a registry holding both this backend's op pack and [C-114](C-114-tool-spec-projection.md)'s
      pack, and assert both install and every name resolves. The two are meant to be used together,
      so a collision between them is the defect most likely to be found by a host rather than by us.
- [ ] A test asserts the op surface is **small and fixed** — the registered tool count does not grow
      with the number of providers. That property is the whole reason this epic exists, so it should
      be enforced rather than assumed.
- [ ] **No network, no socket, no filesystem read at query time.** A test asserting this belongs here.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **Amended 2026-08-04 per flux-roadmap Decision 0006 (rule 9).** This story previously named
  `LiveDatasource`, `try_register_live_datasource` and `ClientBuilder::try_with_live_datasource` —
  a two-op live projection whose method set has no search, relation or batch-get, so the epic's own
  acceptance could not be satisfied by the trait this story named. The compiled-in catalogue binds
  as an indexed backend.
- **Depends on [C-138](C-138-datasource-entity-model.md)** for the model. It composes with
  [C-113](C-113-tool-pack-epic.md)'s crate but does not depend on its request path — this story
  answers about the catalogue and never calls a vendor.
- Where this lives is a real choice: alongside `connector-pack` (both are host-facing flux
  adapters, both register into the host's registry) or in its own crate. Prefer the former unless
  the dependency sets genuinely diverge, and record the reason either way.
- **State the staleness consequence in the docs**, not just the story: a compiled-in catalogue is
  exactly as fresh as the binary. That is correct for this repo's thesis, and it will still surprise
  a host expecting live data.
- The catalogue is read-only here. A datasource that could mutate it would give the generated tree
  two writers and no source of truth — hence the typed refusals above rather than unimplemented
  stubs.
