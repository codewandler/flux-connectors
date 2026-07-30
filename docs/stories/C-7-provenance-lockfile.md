---
id: C-7
title: Record provenance and write connectors.lock
pillar: Spec
status: ready
priority: 11
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec]
---

# Record provenance and write connectors.lock

## Goal
Make drift detectable: every generated artifact records the hashes and versions of everything that
produced it, so upstream movement and stale output are caught rather than silently absorbed.

## Acceptance
- [ ] `Provenance` captures source URL, upstream version, fetched-at, and sha256 of the vendor spec,
      the provider TOML, and the serialized IR.
- [ ] `connectors.lock` serializes one entry per provider, including the generator version, in a
      stable, diff-friendly order.
- [ ] Recomputing hashes over unchanged inputs reproduces the lockfile byte-for-byte — the test that
      makes `check` (`C-14`) trustworthy.
- [ ] Changing any input (spec, TOML, or generator version) changes the corresponding hash and only
      that hash.

## Progress
- (not started)

## Notes
- Depends on `C-2`'s deterministic IR serialization; nondeterminism there shows up here as phantom
  drift on every build.
- The lockfile holds hashes and versions only — never a credential, never a resolved endpoint URL.
