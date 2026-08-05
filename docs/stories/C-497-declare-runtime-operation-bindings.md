---
id: C-497
title: "Declare how connector operations bind to non-HTTP runtimes"
pillar: Spec
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [connector-spec, connector-flux, catalog]
note: "runtime currently names only a kind; a rich operation still has no declared adapter operation or Exchange-consumable stream/lifecycle/result contract"
---

# Declare how connector operations bind to non-HTTP runtimes

## Goal

Extend the connector IR and emitted artefacts so an operation can target a socket, process,
container or plugin adapter without masquerading as an HTTP request or introducing a runtime DSL.

## Acceptance

- [ ] A design fixes the operation binding, input/output/error, cancellation, streaming and lease
      vocabulary for all six runtime kinds.
- [ ] The runtime and binding are connector declarations and cannot be supplied or overridden by a
      caller.
- [ ] A non-HTTP fixture round-trips provider source → IR → manifest/catalogue → embedded Rust
      catalogue with no Exchange component interpreting provider TOML.
- [ ] Invalid combinations fail at load—for example a leased operation with no close/cancel
      contract, or an HTTP-only request shape bound to `plugin`.
- [ ] Existing HTTP connector bytes remain stable unless an intentional versioned schema change is
      recorded.

## Progress

- (not started)

## Notes

- C-405 delivered the runtime kind but deliberately changed no provider and no generated Flux.
- **This story owns the stream/tail/lease spelling — not the one-shot cursor spelling** (noted
  2026-08-04 by C-510; inverted 2026-08-05 by the C-512 contract preflight): the one-shot cursor
  vocabulary already ships as `Pagination::Cursor`'s `cursor_param`, `next_cursor_pointer` and
  `max_pages`, and [C-512](C-512-datasources-ir-member.md) fixes that spelling — this story must
  not mint a second one. C-512's datasource surface
  ([vendor-datasource-declarations.md](../designs/vendor-datasource-declarations.md)) waits on
  this story only for the stream/tail/lease terms: per Decision 0006 it lands with this
  Milestone 2 runtime-declaration work, and streaming datasource reads wait for the Milestone 3
  stream/lease vocabulary this design also fixes.
- C-47 is the SQL lifecycle input; C-489…C-492 are the socket-channel input.
- Exchange is the only official runtime consumer; Flux receives the projected operation through its
  embedded Exchange client and cannot select or execute this binding locally (C-507).
