---
id: C-189
title: "`connectors.lock` is designed, hashed against, and never written"
pillar: Build
status: ready
priority: 3
design:
epic: connectors-v1
areas: [connector-spec, connector-cli]
note: "found by C-184's review: three CHANGELOG entries assert byte-identity of `connectors.lock`, and the file does not exist. `lock.rs` is a complete, tested hash domain whose writer was never built — so provenance is computed and discarded, and drift detection (vision principle 1) is unenforced"
---

# `connectors.lock` is designed, hashed against, and never written

## Goal

Either write the lockfile the design already computes, or stop claiming it exists — so that
`vision.md`'s first principle ("drift is detected, not absorbed") is either enforced or honestly
marked as not yet enforced.

## What was measured

Found by the independent review of [C-184](C-184-auth-scheme-prefix-axis.md), which set out to verify
a byte-identity claim and discovered the artifact was absent:

```
git ls-files | grep -i lock   →  (no connectors.lock)
find . -name 'connectors.lock' →  (nothing)
grep -rn "Lockfile" crates/connector-cli/  →  (nothing)
```

`crates/connector-spec/src/lock.rs` is **not** a stub. It defines the hash domain, `toml_sha256`
(file bytes, comments included, `lock.rs:30`) and `ir_sha256`, and it is exercised by tests. Its own
doc at `lock.rs:48` says writing the file is `connector-cli`'s job. **`connector-cli` never does** —
there is no `Lockfile` construction, no plan entry, no artifact.

So provenance is computed on every build and discarded.

**Three CHANGELOG entries assert properties of this file**, and all three are vacuously true:

| entry | claim |
|---|---|
| `CHANGELOG.md:1072` | "`connectors.lock` entries are unaffected for the 15 providers that declare no inbound members" |
| `CHANGELOG.md:1138` | "no `connectors.lock` entry churns" |
| C-184's entry | corrected in the same commit that filed this story |

## Acceptance

- [ ] **Decide, and record the reason:** write the lockfile, or delete the claim. Both are
      legitimate; what is not legitimate is a third CHANGELOG entry asserting byte-identity of an
      artifact no build produces.
- [ ] **If written:** `build` emits `connectors.lock`, `diff` treats it as any other artifact (a
      stale one fails), and the committed file is a fixed point. The hash domain is `lock.rs`'s as it
      stands — this story does not redesign it.
- [ ] **If written:** a failing-first test asserting the file exists and matches a rebuild. Note the
      ordering hazard — it is a **whole-catalogue** artifact like the index and `catalog.json`, so it
      is coordinator-owned and a `--provider` run must not truncate it (`AGENTS.md`'s scoped-build
      contract).
- [ ] **If deleted:** `lock.rs` says plainly that it is an unused design, the three CHANGELOG
      entries are corrected, and `vision.md` principle 1 says drift detection is **not yet**
      enforced rather than implying it is.
- [ ] Either way, `AGENTS.md`'s *Intentional gaps* names the outcome.

## Progress

- (not started)

## Notes

- **This is a claim-integrity story before it is a feature story.** The repo's own review culture
  found it: a reviewer verifying one byte-identity assertion discovered the artifact was fictional.
  That is the failure mode C-81 (*declared counts are checked*) exists for, in a different surface.
- **`toml_sha256` hashes file bytes with comments included** (`lock.rs:30`). If the lockfile is
  written, the comment-only edit to `providers/launchdarkly.toml` in C-184's commit *would* have
  moved an entry. Worth a test: a comment-only provider edit changes the lock, deliberately, because
  a comment can change what an operator believes the connector does.
- Manifests carry **no** provenance hash today (`connectors/*.connector.toml` has no `sha256`), so
  the lockfile is currently the only proposed home for it. Deleting it means deciding where drift
  detection lives instead — `fetch`/`check` are also unimplemented (`AGENTS.md` *Intentional gaps*).
