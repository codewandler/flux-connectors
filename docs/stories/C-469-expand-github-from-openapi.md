---
id: C-469
title: "Expand GitHub from its official REST OpenAPI description"
pillar: Agent
status: done
priority: 10
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [providers, openapi]
note: "preserve 5 operations; add exact issue, pull-file, workflow-run and commit collection reads"
---

# Expand GitHub from its official REST OpenAPI description

## Goal

Make GitHub spec-backed and add a useful repository automation slice without selector sweeps.

## Acceptance

- [x] Vendor the pinned first-party `github/rest-api-description` document with deterministic scrub,
      provenance and drift tests.
- [x] Failing-first tests pin all five existing operation identities and Flux bytes, then prove at
      least four exact C-468 selectors are added and no unselected operation leaks in.
- [x] New operations carry real response schemas, reviewed query parameters and accurate safety
      metadata; write operations are not inferred from method alone.
- [x] Scoped build/diff and request rehearsal are green; only documented coordinator-owned stale
      catalogue checks may remain red before integration.

## Progress

- 2026-08-02 — failing-first `cargo test -p codewandler-connector-spec --test
  github_spec_selection --test vendored_github_spec` first failed with two missing-selection tests,
  then passed 5/5 after the spec pointer and four exact patches landed.
- 2026-08-02 — `scripts/vendor-github-spec.sh --source-dir <replay>` reproduced identical hashes:
  upstream `281dc9e4ab24860c4010cea1bc90232175a6c92aa73dc569e1ccea6a5018cae9`, vendored
  `6053daae5f41d059b2cb9b857356bc6e847d918f5b569ca203a958118b31c0ed`.
- 2026-08-02 — `cargo run -p connector-cli -- diff --provider github` reported `12 artifacts up to
  date (1 provider checked)`; the GitHub request-rehearsal test passed 1/1 and the five pre-existing
  Flux hashes re-measured byte-identically to the C-468 fence.
- 2026-08-02 — workspace build and clippy were green. The no-fail-fast workspace run reached all
  targets; GitHub's provider, Flux, provenance and request tests were green. Whole-catalogue files
  remain coordinator-owned for C-474, and concurrent Stripe/Graph implementation was still visible
  during that shared-tree run.
