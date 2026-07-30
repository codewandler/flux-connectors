---
id: C-137
title: "The connectors datasource — the catalogue, queryable from a session (epic)"
pillar: Bridge
status: ready
priority: 3
design: docs/designs/connectors-datasource.md
epic: connectors-datasource
areas: [bridge, connector-spec]
note: "EPIC — the Tool pack registers one tool per operation (97 and growing); a datasource is FIVE ops regardless of catalogue size. Discover through the datasource, invoke through the pack. flux's LiveDatasource seam already exists and binds at the same ClientBuilder call"
---

# The connectors datasource — the catalogue, queryable from a session (epic)

## Goal

Let a running flux session ask **"which connector can do this?"** — searching the catalogue and
reading a member's detail — without registering every operation as a tool.

## Acceptance

- [ ] A `LiveDatasource` implementation backed by the `catalog` crate, bound through
      `ClientBuilder::try_with_live_datasource` — the same call a host already uses for the Tool pack.
- [ ] The entity model and its links are declared: provider · service · operation · event · channel
      binding · config field, with the **`oip`** as the record id —
      [C-138](C-138-datasource-entity-model.md).
- [ ] The backend is implemented and registers cleanly beside a Tool pack, with no name collision —
      [C-139](C-139-datasource-backend.md).
- [ ] **Search is good enough to act on**: deterministic ranking, role-aware, and every `Match`
      explains why it matched — [C-140](C-140-datasource-search.md).
- [ ] **Offline and in-process.** No socket, no daemon, no network, and no HTTP API. The source is the
      compiled-in catalogue.

## Children

- [C-138](C-138-datasource-entity-model.md) — the entity model, links, and record ids
- [C-139](C-139-datasource-backend.md) — the `LiveDatasource` implementation and its binding
- [C-140](C-140-datasource-search.md) — search quality: ranking, roles, and explained matches

## Notes

**The scaling argument is the reason to build this.** [C-113](C-113-tool-pack-epic.md)'s pack
registers one tool per operation — 97 today, and the fleet stories multiply it. Every one is
model-facing surface: schema in the context window, a name to disambiguate, a chance to pick wrong. A
datasource is a fixed handful of operations whether the catalogue holds 97 or 970. The two are
complementary: discover through the datasource, invoke through the pack.

**The seam is flux's, and it is already built** — `flux_capabilities::LiveDatasource`,
`try_register_live_datasource` (`crates/flux-capabilities/src/datasource/live.rs:130`), and
`ClientBuilder::try_with_live_datasource` (`crates/flux-sdk/src/lib.rs:549`). The vocabulary is typed
in `flux-datasource`: `Record`, `Link`, `EntitySchema`, `SearchInput`, `Match`, `RelationInput`,
`BatchGetInput`. Nothing is invented here.

**The catalogue is already shaped for it**, which is what the addressing work bought: the `oip`
(`authority[/service]:version#member`) is a stable record id, and a channel binding's link to its
reply operation is the composition [C-82](C-82-channel-bindings-epic.md) already recorded — `Link`
just makes it traversable.

**What this must not become.** "A connectors API" as an HTTP service is
[connectors-proxy.md](../designs/connectors-proxy.md)'s charter question again, gated by
[C-34](C-34-decide-proxy-charter.md). `vision.md` is explicit: no server, no daemon, no request path
of its own. This is a library backend over a committed dataset.

**The consequence to state rather than discover:** a compiled-in catalogue is exactly as fresh as the
binary. Correct for a repo whose whole thesis is that a connector is compiled — but a host expecting
live vendor data will be surprised, so the docs must say so where the surprise would happen.
