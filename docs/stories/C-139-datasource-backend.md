---
id: C-139
title: "The LiveDatasource backend and its binding"
pillar: Bridge
status: ready
priority: 4
design: docs/designs/connectors-datasource.md
epic: connectors-datasource
areas: [bridge]
note: "implements flux's existing LiveDatasource trait over the compiled-in catalogue. Binds through the SAME ClientBuilder call as the Tool pack, so a host configures discovery and invocation in one place"
---

# The LiveDatasource backend and its binding

## Goal

Implement `flux_capabilities::LiveDatasource` over the connectors catalogue, so a host can bind it
with `ClientBuilder::try_with_live_datasource` and query the catalogue from a session.

## Acceptance

- [ ] A backend implementing `LiveDatasource`, reading the **compiled-in `catalog` crate** — not
      `catalog.json` from disk. Offline and deterministic by construction; a runtime file path can be
      missing, stale or edited, for no benefit a rebuild does not already give.
- [ ] It registers through `try_register_live_datasource(registry, domain, backend)`
      (`crates/flux-capabilities/src/datasource/live.rs:130`) and is reachable from
      `ClientBuilder::try_with_live_datasource` (`crates/flux-sdk/src/lib.rs:549`).
- [ ] `get`, `list`, `relation` and `batch_get` are answered from the entity model of
      [C-138](C-138-datasource-entity-model.md). Search is [C-140](C-140-datasource-search.md) and may
      be a naive placeholder here — but say so, and do not let a placeholder ship as if it were the
      feature.
- [ ] **Failing-first test:** `the_datasource_registers_beside_a_tool_pack_without_collision` — build
      a registry holding both this backend and [C-114](C-114-tool-spec-projection.md)'s pack, and
      assert both install and every name resolves. The two are meant to be used together, so a
      collision between them is the defect most likely to be found by a host rather than by us.
- [ ] A test asserts the op surface is **small and fixed** — the registered tool count does not grow
      with the number of providers. That property is the whole reason this epic exists, so it should
      be enforced rather than assumed.
- [ ] **No network, no socket, no filesystem read at query time.** A test asserting this belongs here.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **Depends on [C-138](C-138-datasource-entity-model.md)** for the model. It composes with
  [C-113](C-113-tool-pack-epic.md)'s crate but does not depend on its request path — this story
  answers about the catalogue and never calls a vendor.
- Where this lives is a real choice: alongside `connector-pack` (both are host-facing flux
  adapters, both bind at `ClientBuilder`) or in its own crate. Prefer the former unless the
  dependency sets genuinely diverge, and record the reason either way.
- **State the staleness consequence in the docs**, not just the story: a compiled-in catalogue is
  exactly as fresh as the binary. That is correct for this repo's thesis, and it will still surprise
  a host expecting live data.
- The catalogue is read-only here. A datasource that could mutate it would give the generated tree
  two writers and no source of truth.
