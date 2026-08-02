---
id: C-481
title: Publish per-operation API specification provenance
pillar: Agent
status: done
priority: 2
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [connector-spec, connector-cli, catalogue, web]
note: "mixed-front-end providers require an operation-level source marker; service-level inference is false"
---

# Publish per-operation API specification provenance

## Goal

Let a public catalogue consumer tell which operations were selected from a pinned vendor API
description and identify that source, without exposing repository-local provenance or guessing from
the operation's service.

## Acceptance

- [x] Patch-selected operations retain the exact selected vendor operation id and their document's
      public URL, upstream version, and committed SHA-256 as derived IR provenance; inline operations
      cannot forge that derived marker.
- [x] Every `web/public/catalog.json` operation has a `spec_source` key: `null` for inline operations
      and a stable object for spec-selected ones. Local paths, fetch times, and planning metadata are
      not published.
- [x] Tests cover a fully inline provider, a spec-backed provider, and Zendesk's mixed Support and
      Messaging services so source classification cannot regress to service-level inference.
- [x] The web consumer's checked type accepts and exposes the new additive field, and generated
      catalogue artifacts remain deterministic.
- [x] Any public Rust-API compatibility impact is recorded in the engineering and customer
      changelogs before the minor release.

## Progress

- 2026-08-02: filed when C-474's independent integration review proved that provider and operation
  entries in the public catalogue had no source field despite C-467 requiring the distinction.
- 2026-08-02: patch application now records a `BTreeMap` from public operation id to the exact
  selected vendor operation and document. The record measures the committed bytes it actually
  ingested, reads the upstream version from that document, permits only the public URL to be null,
  and stays outside the IR hash domain. Because it lives in derived `Provenance` rather than
  `Operation`, provider TOML cannot author it and existing `Operation` literals do not change.
- 2026-08-02: the catalogue emitter publishes `spec_source` on every operation and a four-field
  object only for selected operations. Focused evidence: `cargo test -p
  codewandler-connector-spec --test operation_spec_source --test provider_schema --test
  ir_roundtrip --test lockfile --no-fail-fast` passed 40 tests; `cargo test -p connector-cli
  site::tests --lib --no-fail-fast` passed 18; `npm ci && npm run build` passed in `web/`; the focused
  type-check mechanism passed; and `cargo clippy --workspace --all-targets -- -D warnings` plus
  `cargo fmt --all --check` passed. The full web suite passed 41 of 42 and reported only the expected
  coordinator-owned stale `web/public/catalog.json`, which a full build will regenerate at wave
  integration.
- 2026-08-02: compatibility is explicit in both changelogs. `OperationSpecSource` is additive, but
  adding `Provenance::operation_specs` breaks downstream direct struct literals and exhaustive
  destructuring; they must add an empty map or use `..Provenance::default()`.
