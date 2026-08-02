---
id: C-471
title: "Expand Microsoft Graph from its official v1.0 OpenAPI metadata"
pillar: Agent
status: done
priority: 12
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [providers, openapi]
note: "preserve 8 operations; extract one message and three Outlook calendar-metadata reads"
---

# Expand Microsoft Graph from its official v1.0 OpenAPI metadata

## Goal

Make the Microsoft Graph connector spec-backed while keeping its mail, calendar and files service
addresses stable.

## Acceptance

- [x] Vendor the pinned first-party `microsoftgraph/msgraph-metadata` v1.0 contract, or a deterministic
      reference-closed extraction whose provenance includes the full upstream hash.
- [x] Failing-first tests pin the eight existing operations/Flux bytes and prove at least four exact
      C-468 selectors, their service assignments and scopes are added without a sweep.
- [x] OData query parameters are individually curated; only integer `$top`/`$skip` paging survives,
      while `$select`, `$filter`, `$expand` and string queries are omitted.
- [x] Mail-sending or calendar mutation carries explicit external effects and high risk; otherwise the
      story substitutes documented reads rather than weakening metadata.
- [x] Scoped build/diff and request rehearsal are green.

## Progress

- 2026-08-02 — failing-first evidence: `CARGO_INCREMENTAL=0 cargo test -p
  codewandler-connector-spec --test microsoft_graph_spec_selection --no-fail-fast` reported four
  failures before the spec pointers, exact selectors and extracts existed; the eight existing Flux
  hashes already passed.
- 2026-08-02 — the deterministic replay check reported `36 reference-closed components`; the
  extracted inputs measured 173,979 bytes for mail and 155,454 bytes for calendar, while provenance
  retains the measured 38,050,122-byte upstream document and its
  `2749e51f363a471cdaa4835493c2c57198aa834262666da39c03a2e7f9f9d831` SHA-256.
- 2026-08-02 — `CARGO_INCREMENTAL=0 cargo run -p connector-cli -- diff --provider microsoft_graph` reported
  `19 artifacts up to date (1 provider checked)`; the six-test selection/provenance binary and the
  four-operation request rehearsal both passed. Full-workspace and whole-catalogue integration stay
  coordinator-owned under C-474.
- 2026-08-02 — `CARGO_INCREMENTAL=0 cargo test -p connector-flux --test
  microsoft_graph_connector --no-fail-fast` passed all 12 provider-specific tests. Targeted clippy
  also passed for that binary, `microsoft_graph_spec_selection`, and
  `microsoft_graph_rehearsal` with `-D warnings`.
