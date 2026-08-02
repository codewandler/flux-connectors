---
id: C-461
title: "Add query-free Zendesk ticket audit history from the Ticketing spec"
pillar: Spec
status: done
priority: 1
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [providers, connector-flux]
note: "the first real selection from Zendesk's full OAS: one useful response-shaped operation, with every unsafe optional query parameter explicitly omitted"
---

# Add query-free Zendesk ticket audit history from the Ticketing spec

## Goal
Add one useful, response-shaped Support operation through opt-in Ticketing OAS selection without
moving or weakening the seven operations that already ship.

## Acceptance
- [x] A failing-first provider test selects `ListAuditsForTicket` as
      `zendesk-ticket-audit-list` and proves its exact method, path, numeric ticket id, response
      schema, low risk, and idempotency.
- [x] All seven optional query parameters — `page`, `sort`, `include`,
      `include_boundary_indicators`, `include_item_cursors`, `filter_events`, and `sort_order` — are
      explicitly omitted; the emitted request is query-free until query encoding is safe.
- [x] The provider points at C-459's pinned Ticketing document with its recorded public provenance;
      selection is one operationId, never a path/tag sweep.
- [x] The original seven operation ids, methods, paths, OIPs, and per-operation Flux remain pinned.
- [x] Scoped build/diff emits the new per-operation artifact, pack rehearsal composes it from declared
      config, and the workspace gate has no provider-specific red beyond coordinator-owned catalogue
      staleness.

## Progress

- Failing first: `cargo test -p codewandler-connector-spec --test zendesk_spec_selection
  --no-fail-fast` ran three tests; the two new selection assertions failed on seven operations versus
  eight and zero patches versus one, while the original-Flux pin passed.
- After the provider change,
  `cargo test -p codewandler-connector-spec --test zendesk_spec_selection --test
  vendored_zendesk_specs --no-fail-fast` passed all eight focused tests: three selection tests and
  five vendored-document tests.
- `cargo run -q -p connector-cli -- build --provider zendesk` reported
  `1 provider, 11 artifacts; 4 written`; the following scoped diff reported
  `11 artifacts up to date (1 provider checked)`.
- `cargo test -p codewandler-connector-pack --test request
  every_declared_operation_composes_a_request_from_its_declared_configuration -- --exact
  --nocapture` passed its one request-composition rehearsal.
- `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo fmt --all --check` passed.
- The first `cargo test --workspace --no-fail-fast` exposed two integration regressions beyond
  generated state: the curated census still expected seven operations, and C-459's provider-prefixed
  vendored-spec test tripped the per-provider catalogue-walk fence. Updating the census to eight and
  renaming the test without suppressing either assertion made both targets green.
- The final `cargo test --workspace --no-fail-fast` reported exactly five failed targets, all naming
  coordinator-owned whole-catalogue state: `catalog_artifacts`, `lockfile`, `readme_snippet`,
  `service_units`, and `site_catalog`. The Zendesk selection, vendored documents, curated census,
  per-provider scope fence, response-shape gates, and request-composition gate were green in that
  workspace run.
- Coordinator integration ran the full build, which wrote `connectors.lock` and
  `web/public/catalog.json`; the following full diff reported
  `952 artifacts up to date (54 providers checked)`, and the full no-fail-fast workspace test then
  exited green.

## Notes
- C-6 still owns converting the seven inline operations and measuring patch cost. This story mixes
  their unchanged inline definitions with one spec-selected operation so that work is not conflated.
