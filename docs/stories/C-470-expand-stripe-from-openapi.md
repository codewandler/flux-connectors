---
id: C-470
title: "Expand Stripe from its official OpenAPI description"
pillar: Agent
status: done
priority: 11
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [providers, openapi]
note: "preserve 8 operations; add exact country-spec, event, exchange-rate and billing-meter reads"
---

# Expand Stripe from its official OpenAPI description

## Goal

Make Stripe spec-backed and broaden safe billing visibility without accidentally exposing money-
moving endpoints.

## Acceptance

- [x] Vendor the pinned first-party `stripe/openapi` GA document with deterministic scrub,
      provenance and drift tests.
- [x] Failing-first tests pin all eight existing operations/Flux bytes and prove at least four exact
      amended C-468 selectors are added with no selector sweep; the original four selectors remain
      measured `$ref`-cycle deferrals rather than hand-truncated schemas.
- [x] List pagination/filter inputs are curated; response schemas are retained; every money-moving
      candidate remains withheld unless its `money` effect and approval posture are explicitly proven.
- [x] Scoped build/diff and request rehearsal are green.

## Progress

- 2026-08-02: the failing-first selection test initially reported `left: 0, right: 4` for the absent
  exact patches while all eight original Flux-byte pins passed.
- 2026-08-02: re-running `scripts/vendor-stripe-spec.sh --fetched-at
  2026-08-02T11:12:55Z` recorded the pinned upstream SHA-256
  `6f3623aece40493eec2f5e3e631219f8c6bffa4f477e3807a4bf785ad377f237` and reproduced the vendored
  SHA-256 `3b3d858f1a02a2ac0116cc10a296a7d374fe7c9d8b4a68a5419712e0bb3fbf41`.
- 2026-08-02: `cargo run -p connector-cli -- diff --provider stripe` finished with
  `15 artifacts up to date (1 provider checked)`; the targeted spec-selection and Stripe connector
  suites passed 4/4 and 8/8, and
  `cargo test -p codewandler-connector-pack --test request
  every_declared_operation_composes_a_request_from_its_declared_configuration` passed 1/1.
