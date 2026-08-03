---
id: C-505
title: "Gate the complete native-plugin migration and locality parity"
pillar: Bridge
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [tests, catalog, migration]
note: "one ratcheted inventory maps all 18 integration crates to connectors and requires frozen behavioral parity locally and through Exchange before a Flux crate disappears"
---

# Gate the complete native-plugin migration and locality parity

## Goal

Make “all integrations are connectors” a checked migration rather than a roadmap sentence: every
official native adapter is accounted for, parity evidence is retained, and local/hosted behavior
cannot drift silently.

## Acceptance

- [ ] A checked inventory reads the Flux plugin manifests and classifies support crates separately;
      every integration crate maps to exactly one connector and migration wave.
- [ ] A shared conformance format freezes operation/event schemas, effects, capability subjects,
      results, errors, cancellation and stream/lease behavior without copying implementation details.
- [ ] Each migrated connector runs the applicable suite locally and through Exchange; an unsupported
      locality is an explicit refusal with a reason, not a skipped test.
- [ ] Removing a Flux integration crate before its connector is published and conformant fails the
      cross-repository release checklist.
- [ ] The epic closes only when the official integration adapter count under `flux/plugins` is zero;
      generic protocol/support crates are reported separately.

## Progress

- (not started)
