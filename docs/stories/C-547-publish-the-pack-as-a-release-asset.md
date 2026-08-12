---
id: C-547
title: "Publish the catalog pack as a verifiable release asset"
pillar: Build
status: ready
priority: 1
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [release, catalog]
note: "Operator-proposed 2026-08-12: attach catalog.pack (+ out-of-band sha256) to the GitHub release so a client fetches the catalogue without cargo; Pack::load(path) already refuses a wrong digest or schema version, so the verification half exists"
---

# Publish the catalog pack as a verifiable release asset

## Goal

A client that wants the catalogue — Exchange loading a newer catalogue than it was built with, or
any consumer with no Rust toolchain — fetches `catalog.pack` straight from the GitHub release and
verifies it, with no cargo and no clone. The reader's `Pack::load(path)` already refuses a wrong
digest or schema version before serving a record; this story gives that constructor a supported
place to fetch from.

## Acceptance

- [ ] Every `vX.Y.Z` release carries `catalog.pack` and `catalog.pack.sha256` as assets, attached
      **mechanically by the tag-triggered workflow** (the crates.io workflow or a sibling on the
      same trigger), never by hand — the asset is the committed pack at the tag, byte-identical,
      and the workflow fails loudly if the digest it computes disagrees with the tag's
      `connectors.lock` `[pack]` row.
- [ ] The out-of-band digest is the in-band one: `catalog.pack.sha256` equals the digest embedded
      in the pack and the lockfile row, so a client can verify before and the reader verifies
      after, against the same value.
- [ ] The fetch-and-load client contract is documented where a consumer looks — the reader crate's
      README (the release-asset URL shape, the sha256 check, `Pack::load`'s refusals) — without
      publishing repository internals on the public site.
- [ ] The workflow half is pinned by a test in the repository's existing style (the
      `web/test/ci_gate.test.mjs` pattern of asserting a workflow enforces what the docs claim, or
      a Rust test over the workflow file), so the attachment cannot be narrowed away silently.
- [ ] v0.22.0's assets exist (attached at integration if the coordinator has not already done it),
      so the contract holds for the release Exchange starts from, not only future ones.

## Progress

- 2026-08-12: Filed from the operator's proposal during the v0.23.0 session. The schema-version
  and digest refusals this leans on shipped in C-537.

## Notes

- Write set: `.github/workflows/crates-io.yml` (or a new tag-triggered workflow file),
  `crates/catalog-reader/README.md`, plus the pinning test. Deliberately does NOT touch
  `scripts/cut-release.sh` or `.github/workflows/ci.yml`, so it stays wave-disjoint from C-543.
- The pack is ~9.5 MB raw; assets are served by GitHub's CDN. No compression decision is reopened
  here — the asset is the committed artifact, byte-identical.
