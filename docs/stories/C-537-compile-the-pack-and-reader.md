---
id: C-537
title: "Compile the pack and publish the reader"
pillar: Build
status: done
priority: 1
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-cli, catalog, release]
note: "One versioned, digest-carrying compressed pack derived from the canonical documents; a dependency-free reader preserving the catalog API; the catalog crate becomes a shim over the embedded pack"
---

# Compile the pack and publish the reader

## Goal

Compile the canonical documents into one compressed, indexed, digest-carrying file, and serve it
through a dependency-free reader that preserves today's `catalog` API — so catalogue data stops
riding code releases while every existing consumer keeps compiling.

## Acceptance

- [x] `flux-connectors build` derives a single pack file byte-deterministically from the canonical
      documents; the pack embeds its schema version and content digest, and the lockfile records
      it as a whole-catalogue artifact.
- [x] The container format decision (working choice: zstd-compressed canonical CBOR with a
      provider/operation offset index) is recorded in the design with the measured size and read
      cost that justified it; a competing option is rejected in writing, not silently.
- [x] A reader crate with zero non-optional dependencies exposes `providers()`, `provider()`,
      `operation()`, `operations_of()` over the embedded pack, plus a `load(path)` constructor
      that refuses a wrong schema version or digest before serving any record.
- [x] `codewandler-connector-catalog` becomes a shim over the embedded pack with **no breaking
      change to its public API** — `crates/connector-cli/tests/publish_closure.rs` and a
      consumer-side compile test hold that line.
- [x] The generated Rust in `crates/catalog/src/generated/` is no longer the storage: either it is
      reduced to the embed + re-export, or its removal is explicitly deferred to C-540 with the
      reason recorded here. **Taken by deferral to C-540**: `Operation::flux` is promised public
      API, the documents deliberately carry no Flux, `connector-pack` still parses it, and the
      differential gate (C-538) needs the Flux-derived plan intact — reducing the tables now would
      re-embed the text the artifact exists to retire. Full reasoning in
      `docs/designs/catalog-artifact.md` §2.4.
- [x] Publishing: the reader joins the derived publish closure; the closure derivation and
      dry-run gate in `scripts/publish-crates-io.sh` cover it.

## Progress

- 2026-08-12 — Implemented on `impl/C-537` (six commits plus review follow-up `a853e2fb`), merged
  `b3a47f6b`. Container decision: uncompressed offset-indexed UTF-8 over the committed document
  bytes; the zstd-CBOR working choice rejected in writing (design §2.1–2.3 — a zero-dependency
  reader can carry no codec, compression ties byte-determinism to a compressor version, and the
  raw payload keeps every record byte-identical to its reviewed committed document). Determinism:
  `sha256sum crates/catalog-reader/catalog.pack` → `7670fe86…` reproduced across three independent
  builds in two checkouts; `connectors.lock` records the pack under `[pack]`. Independent review:
  PASS, no blocking findings; three minors fixed on-branch (sha256 doc pointer, index-cost
  measurement corrected to 52,467 B ≈ 0.55%, inbound/config surfaces pinned in
  `consumer_api.rs`). The build now plans 1167 artifacts (55 providers).

## Notes

- Depends on C-536's documents existing; do not share a wave with C-536.
- Forward compatibility is part of the schema contract: an additive field must not break an older
  reader; a reader must refuse a newer major, fail-closed, by name.
