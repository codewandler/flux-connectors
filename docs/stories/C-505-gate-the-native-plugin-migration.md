---
id: C-505
title: "Establish the native-plugin migration inventory and Exchange conformance ratchet"
pillar: Bridge
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [tests, catalog, migration]
note: "an atomic prerequisite maps all 18 adapters and establishes frozen legacy-plugin-versus-Exchange fixtures before the first wave; each wave ratchets its own evidence"
---

# Establish the native-plugin migration inventory and Exchange conformance ratchet

## Goal

Before the first adapter wave begins, make “all integrations are connectors” a checked migration:
account for every official native adapter and establish the reusable frozen-fixture format that each
connector must pass through Exchange before Flux deletes its legacy implementation.

## Acceptance

- [ ] A checked inventory reads the Flux plugin manifests and classifies support crates separately;
      every integration crate maps to exactly one connector and migration wave.
- [ ] A shared conformance format freezes operation/event schemas, effects, capability subjects,
      results, errors, cancellation and stream/lease behavior without copying implementation details.
- [ ] The harness compares the legacy Flux plugin with Exchange execution and distinguishes an
      unsupported runtime or topology as an explicit refusal rather than a skipped test.
- [ ] Removing a Flux integration crate before its connector is published and conformant fails the
      cross-repository release checklist.
- [ ] The initial ratchet lands before C-499; C-499…C-503 add their own adapters and retained evidence
      in fixed wave order without waiting for one global cutover.
- [ ] Completion of this story means the inventory, format and ratchet exist—not that all adapters
      have migrated. C-495 closes only when the official integration adapter count reaches zero.

## Progress

- (not started)

## Notes

- C-507 split this atomic foundation from the long-running epic closure, removing the semantic cycle
  where the migration evidence depended on a journey that itself required the evidence corpus.
