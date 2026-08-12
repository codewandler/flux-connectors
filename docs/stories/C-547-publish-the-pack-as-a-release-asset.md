---
id: C-547
title: "Publish the catalog pack as a verifiable release asset"
pillar: Build
status: in-progress
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

- [x] Every `vX.Y.Z` release carries `catalog.pack` and `catalog.pack.sha256` as assets, attached
      **mechanically by the tag-triggered workflow** (the crates.io workflow or a sibling on the
      same trigger), never by hand — the asset is the committed pack at the tag, byte-identical,
      and the workflow fails loudly if the digest it computes disagrees with the tag's
      `connectors.lock` `[pack]` row.
- [x] The out-of-band digest is the in-band one: `catalog.pack.sha256` equals the digest embedded
      in the pack and the lockfile row, so a client can verify before and the reader verifies
      after, against the same value.
- [x] The fetch-and-load client contract is documented where a consumer looks — the reader crate's
      README (the release-asset URL shape, the sha256 check, `Pack::load`'s refusals) — without
      publishing repository internals on the public site.
- [x] The workflow half is pinned by a test in the repository's existing style (the
      `web/test/ci_gate.test.mjs` pattern of asserting a workflow enforces what the docs claim, or
      a Rust test over the workflow file), so the attachment cannot be narrowed away silently.
- [x] v0.22.0's assets exist (attached at integration if the coordinator has not already done it),
      so the contract holds for the release Exchange starts from, not only future ones.

## Progress

- 2026-08-12: Filed from the operator's proposal during the v0.23.0 session. The schema-version
  and digest refusals this leans on shipped in C-537.
- 2026-08-12: Implemented as `.github/workflows/release-assets.yml` — a sibling workflow on the
  same `v[0-9]+.[0-9]+.[0-9]+` tag trigger, not a job in `crates-io.yml`. Three reasons, all about
  the publish: an attachment must not be able to fail an irreversible step, must not queue in the
  `crates-io-publish` concurrency group ahead of a `workflow_dispatch` resume, and needs
  `contents: write` where that workflow deliberately runs at `contents: read`. `crates-io.yml` is
  untouched.
- 2026-08-12: **Ordering.** The release object usually does not exist at tag time — § Release
  process creates it after the publish is green — so the workflow also triggers on
  `release: published` and re-runs the same job against `refs/tags/<tag>`, uploading with
  `--clobber`. It never creates a release itself: a draft created here would make the documented
  `gh release create` fail. The digest check runs on every trigger, so a pack that disagrees with
  its tag is red at tag time, before a release exists to carry it.
- 2026-08-12: **Acceptance bullet 2 is true of the value that matters and false as literally
  worded.** `catalog.pack.sha256` equals the lockfile `[pack]` row exactly (`e6c1f242…` at v0.22.0,
  and the staged asset is byte-identical to the one already published). It cannot equal the pack's
  *embedded* digest (`855f40d9…`): the header's `digest` line covers every byte after itself, so by
  construction it is a different number from the whole-file one. The workflow therefore checks
  both — whole file against the lockfile, header against its own content — and the README states
  the two-digest split rather than papering over it.

## Notes

- Write set: `.github/workflows/crates-io.yml` (or a new tag-triggered workflow file),
  `crates/catalog-reader/README.md`, plus the pinning test. Deliberately does NOT touch
  `scripts/cut-release.sh` or `.github/workflows/ci.yml`, so it stays wave-disjoint from C-543.
- The pack is ~9.5 MB raw; assets are served by GitHub's CDN. No compression decision is reopened
  here — the asset is the committed artifact, byte-identical.
