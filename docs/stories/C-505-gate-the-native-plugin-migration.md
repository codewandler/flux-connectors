---
id: C-505
title: "Establish the native-plugin migration inventory and Exchange conformance ratchet"
pillar: Bridge
status: done
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [tests, catalog, migration]
note: "MILESTONE 1 lane 15 — freeze the complete native-adapter inventory and reusable legacy-versus-Exchange conformance format before the first migration wave"
---

# Establish the native-plugin migration inventory and Exchange conformance ratchet

## Goal

Before the first adapter wave begins, make “all integrations are connectors” a checked migration:
account for every official native adapter and establish the reusable frozen-fixture format that each
connector must pass through Exchange before Flux deletes its legacy implementation.

## Acceptance

- [x] A checked inventory reads the Flux plugin manifests and classifies support crates separately;
      every integration crate maps to exactly one connector and migration wave.
- [x] A shared conformance format freezes operation/event schemas, effects, capability subjects,
      results, errors, cancellation and stream/lease behavior without copying implementation details.
- [x] The harness compares the legacy Flux plugin with Exchange execution and distinguishes an
      unsupported runtime or topology as an explicit refusal rather than a skipped test.
- [x] Removing a Flux integration crate before its connector is published and conformant fails the
      cross-repository release checklist.
- [x] The initial ratchet lands before C-499; C-499…C-503 add their own adapters and retained evidence
      in fixed wave order without waiting for one global cutover.
- [x] Completion of this story means the inventory, format and ratchet exist—not that all adapters
      have migrated. C-495 closes only when the official integration adapter count reaches zero.

## Progress

- 2026-08-03 — Completed the retained inventory, closed conformance schema, derived comparator,
  publication receipt gate and offline `migration-check` command. Failing-first evidence:
  `cargo test -p connector-cli --test native_plugin_migration` first failed with unresolved
  `connector_cli::migration`; after implementation the same command reported `9 passed; 0 failed`.
- 2026-08-03 — Re-fetched Flux and ran
  `git -C /home/timo/projects/flux rev-parse origin/main`, which printed
  `3da5b9771e97f41540d91856dd0273caad809662`; then
  `cargo run -p connector-cli -- migration-check --flux-root /home/timo/projects/flux` reported
  `18 inventoried; 18 legacy present; 0 retired with evidence; 2 support crates`.
- 2026-08-03 — Repository gate passed with
  `cargo build --workspace && cargo test --workspace --no-fail-fast && cargo clippy --workspace
  --all-targets -- -D warnings && cargo fmt --all --check`.
- 2026-08-03 — Implementation started from canonical connectors commit
  `0a56255a335a0bb812580dbcaaec24d6168ba10a`; inventory, conformance-format and release-ratchet
  investigations are running against the current sibling contracts.
- 2026-08-03 — Scheduled as the active Milestone 1 connectors lane at roadmap commit
  `8a653d222b86fe420c551ca8fd366a602ad6c26a`; this preparation changes board state only, and
  Acceptance remains unimplemented.

## Notes

- C-507 split this atomic foundation from the long-running epic closure, removing the semantic cycle
  where the migration evidence depended on a journey that itself required the evidence corpus.
