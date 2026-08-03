---
id: C-497
title: "Declare how connector operations bind to non-HTTP runtimes"
pillar: Spec
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [connector-spec, connector-flux, catalog]
note: "runtime currently names only a kind; a rich operation still has no declared adapter operation, stream/lifecycle shape or host-neutral result contract"
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
      catalogue with no host interpreting provider TOML.
- [ ] Invalid combinations fail at load—for example a leased operation with no close/cancel
      contract, or an HTTP-only request shape bound to `plugin`.
- [ ] Existing HTTP connector bytes remain stable unless an intentional versioned schema change is
      recorded.

## Progress

- (not started)

## Notes

- C-405 delivered the runtime kind but deliberately changed no provider and no generated Flux.
- C-47 is the SQL lifecycle input; C-489…C-492 are the socket-channel input.
