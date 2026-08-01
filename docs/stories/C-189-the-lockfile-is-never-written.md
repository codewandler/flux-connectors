---
id: C-189
title: "`connectors.lock` is designed, hashed against, and never written"
pillar: Build
status: done
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

- [x] **Decide, and record the reason:** write the lockfile, or delete the claim. Both are
      legitimate; what is not legitimate is a third CHANGELOG entry asserting byte-identity of an
      artifact no build produces. → **Written.** See *Progress* for the reason.
- [x] **If written:** `build` emits `connectors.lock`, `diff` treats it as any other artifact (a
      stale one fails), and the committed file is a fixed point. The hash domain is `lock.rs`'s as it
      stands — this story does not redesign it. → `crates/connector-cli/src/pipeline.rs:129`
      (the plan entry) and `pipeline.rs:200` (`lock_entry`); `connectors.lock` is committed and
      `cargo run -p connector-cli -- diff` reports `558 artifacts up to date (53 providers checked)`.
- [x] **If written:** a failing-first test asserting the file exists and matches a rebuild. Note the
      ordering hazard — it is a **whole-catalogue** artifact like the index and `catalog.json`, so it
      is coordinator-owned and a `--provider` run must not truncate it (`AGENTS.md`'s scoped-build
      contract). → `crates/connector-cli/tests/lockfile.rs`, seven tests; the scoping half is
      `a_scoped_build_leaves_the_lockfile_byte_identical`.
- [ ] **If deleted:** `lock.rs` says plainly that it is an unused design, the three CHANGELOG
      entries are corrected, and `vision.md` principle 1 says drift detection is **not yet**
      enforced rather than implying it is. → not applicable; the lockfile was written.
- [x] Either way, `AGENTS.md`'s *Intentional gaps* names the outcome. → the gap is recorded as
      CLOSED, with the two things that remain open named (C-14's verifier, C-25's
      `upstream_spec_sha256`).

## Progress

**Written, not deleted.** The deciding fact is that the repository has since acquired a reason to
need it: babelforce (C-416) is the first spec-backed connector, it pins a vendored document by path
and `sha256`, and the standing direction is to source more providers from OpenAPI documents
directly. Every one of those depends on being able to answer "has the upstream document moved, and
is the artifact still the one that document produced". Deleting the design would have meant deciding
where that answer lives instead, on the same day the first connector started needing it.

What landed:

- **`connectors.lock` is a planned artifact**, emitted on a full run only, and committed (907 lines,
  53 rows). It is the 558th artifact; the count in `AGENTS.md` moved with it.
- **Artifact hashes are keyed by repository-relative path**, not by bare file name as `lock.rs`'s
  docstring example had it. `check` has to *find* a file to rehash it, and bare names collide across
  the three directories a provider emits into — an operation named after its provider would render
  `crates/catalog/ops/<id>/<id>.flux` under the same key as `connectors/<id>.flux`, silently
  dropping one hash. `Workspace::artifact_key` normalises the separator so the key is the same on
  every platform.
- **`LockEntry::specs`** — a per-document row list, mirroring `Provenance::specs` (C-410). This is
  the one addition beyond "write the file the design computes", and it is here rather than in C-14
  because a connector compiling from several documents has `spec_sha256 == None` and would otherwise
  be recorded with **no spec hash at all** — a row that looks complete and detects nothing. It is
  `LockSpec`, not `SpecSource`, so `fetched_at` is dropped at a named boundary: a re-vendor of
  byte-identical bytes must not rewrite the lockfile.
- Nothing in the catalogue exercises the multi-document case today — babelforce pins **one** of its
  five vendored documents — so the guarantee is held by a fixture,
  `a_connector_compiled_from_several_documents_records_a_hash_for_each`.

Measured, not predicted: a new provider now leaves **nine** tests red across **six** binaries, not
eight across five. The ninth is `the_committed_lockfile_is_a_fixed_point_of_a_build`, and it clears
at integration exactly as the other eight do.

Deliberately not done, and named in `AGENTS.md`: `upstream_spec_sha256` is still filled by nobody
(it needs `specs/<vendor>.provenance.toml` wired in — C-25), and no whole-catalogue artifact is in
any row, because a `LockEntry` is one provider's.

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
