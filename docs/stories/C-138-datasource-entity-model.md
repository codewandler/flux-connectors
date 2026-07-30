---
id: C-138
title: "The datasource entity model, its links, and the oip as record id"
pillar: Bridge
status: ready
priority: 3
design: docs/designs/connectors-datasource.md
epic: connectors-datasource
areas: [bridge, connector-spec]
note: "the addressing work already bought this — the oip (authority[/service]:version#member) is a stable record id, and a binding's link to its reply operation is C-82's composition made traversable"
---

# The datasource entity model, its links, and the oip as record id

## Goal

Declare what a record *is* in the connectors datasource, and what links one record to another, using
flux's existing `EntitySchema` / `Record` / `Link` vocabulary.

## Acceptance

- [ ] Entity kinds are declared: **provider · service · operation · event · channel binding · config
      field**. A kind the catalogue does not yet publish (graphs, until they reach an artifact) is
      **omitted**, not stubbed — an entity that returns nothing is worse than one that is absent.
- [ ] The record id is the **`oip`** (`authority[/service]:version#member`) from
      [C-37](C-37-global-addressing.md), not a synthesised key. A provider with no declared authority
      has no rendered address, so decide and record what its records are keyed by rather than
      inventing an id that no other artifact uses.
- [ ] `EntitySchema` per kind, derived from the IR's declared types — never hand-written, or it
      becomes a third place the shape is stated.
- [ ] Links are declared and traversable with `RelationInput`: provider→service, service→member,
      **binding→reply operation**, operation→credential, operation→host.
- [ ] **Failing-first test:** `every_catalogue_member_is_reachable_as_a_record` — walk the shipped
      catalogue and assert each member resolves to a record with a stable id, and that every declared
      link resolves to an existing record. A dangling link must fail it.
- [ ] A test asserts the model is **non-empty and multi-kind** — at least two entity kinds with
      records — so the walk cannot pass vacuously.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- The vocabulary is flux's and already typed: `Record`, `Link`, `SchemaField`, `EntitySchema`,
  `Declaration` in `crates/flux-datasource/src/lib.rs`. Read them before modelling anything — this
  story implements an existing shape rather than designing one.
- **The binding→reply link is the interesting one.** [C-82](C-82-channel-bindings-epic.md) already
  established that a channel binding *is* a composition of an event and a reply operation, and the
  reply is already published as a rendered `oip`. `Link` is what turns that from documentation into
  something a caller can traverse.
- Keep credential **names** and never values, as everywhere else. A datasource that can walk from an
  operation to its credential must reach the credential's *declaration*, not its secret.
- This story models; [C-139](C-139-datasource-backend.md) implements the backend and
  [C-140](C-140-datasource-search.md) makes search worth using. Landing the model alone is fine and
  reviewable on its own.
