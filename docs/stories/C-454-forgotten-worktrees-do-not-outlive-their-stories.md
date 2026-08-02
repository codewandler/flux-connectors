---
id: C-454
title: "Forgotten worktrees do not outlive their stories or the release that should contain them"
pillar: Build
status: done
priority: 1
areas: [build]
note: "release audit 2026-08-02: C-403's obsolete rollback was pruned; concurrently-created C-455 was recovered as commit 6e421af; five completed stories and three blocker chains were reconciled before the v0.13.0 cut"
---

# Forgotten worktrees do not outlive their stories

## Goal

Account for every recoverable worktree change before release, and make the generated board agree with
the work that actually landed.

## Acceptance

- [x] Inventory registered worktrees, filesystem `.git` links, merged and unmerged local branches,
      refs, reflogs and unreachable commit tips. Every count in the report comes from this session.
- [x] Review every recoverable dirty snapshot against current `main`: integrate work that is still
      valid; record why a stale or superseded patch must not be replayed.
- [x] Audit `in-progress` and `blocked` stories against their Acceptance, Progress and merged branch
      history. Close completed stories and update blockers that another completed story removed.
- [x] No forgotten worktree metadata or unmerged local commit remains, and merged `worktree-agent-*`
      refs no longer masquerade as active work.
- [x] Regenerate the board, add the changelog entry, and run the full generated-artifact, Rust, web
      and host UI gates before cutting the release.

## Progress

- The initial `git worktree list --porcelain` reported one worktree: the primary `main` checkout.
- `find . -path '*/.git' -type f` reports no unregistered linked checkout under the repository.
- `git branch --no-merged main` reports no branch; every local branch head is an ancestor of `main`.
- `git fsck --full --no-reflogs --unreachable` reports 98 unreachable commits. Removing commits that
  are parents of another unreachable commit leaves 47 recoverable graph tips. Four further commits
  are reachable only through reflogs; all four are among those unreachable objects.
- The removed C-403 checkout was recovered through `salvage/c403-worktree`; a fresh snapshot of its
  last tree was byte-identical before removal. After the review below, the tag and object were pruned.

### Recoverable-object review

All 47 unreachable graph tips were compared to their first parent, merged conceptually with current
`main` using `git merge-tree`, and traced to the story and current files they touched.

- Empty, exact or already-subsumed snapshots add no tree change.
- Work-in-progress snapshots for C-27, C-53, C-54, C-60, C-74, C-83, C-96, C-100, C-101, C-102,
  C-109, C-115, C-116, C-120, C-150, C-164, C-168, C-175, C-187, C-197, C-199, C-207, C-220,
  C-223 and C-239 all point at stories now `done`; their later implementations and tests are on
  `main`. The ordinary unreachable commits for C-60's verification vectors, C-187's pins and
  C-199/C-223's network fence are likewise superseded by the current versions of those files.
- Coordinator snapshots that contain old whole-catalogue output or release edits are stale by
  construction. Current generated artifacts, board and changelogs are regenerated from their
  sources and proved by the release gate rather than recovered from an old tree.
- The C-403 patch is actively unsafe to replay: it removes `flux_engine_line.rs`, moves the Flux
  requirement backwards and rewrites the canonical response record back to the pre-0.43 flat-string
  contract. Current `main` is on engine line 0.49 and pins the record in both documentation and
  `live_egress`.
- Commit `54ef636`, the unsanitized babelforce document C-25 explicitly forbade merging, is unreachable
  from every branch and tag. C-415's scrubbed documents are the integrated version; the abandoned
  object is removed before release rather than made reachable.

### Story and ref reconciliation

- C-25, C-158, C-240, C-407 and C-416 now say `done`, matching their implementations and Progress.
- C-191 and C-404 are `ready`: C-158/C-205 and C-403 respectively removed their recorded blockers.
  C-238 remains blocked only on C-191. C-133 records C-206's completed unauthenticated-operation
  contract while retaining its truthful-output and acceptable-use blockers.
- C-82, C-90, C-94, C-119 and C-124 now check the child work that is already done. The active output
  stories no longer claim that the current `http.request` returns a flat string.
- Immediately before pruning, `git branch --no-merged main` returned zero. All 75 non-main local
  branches were therefore deleted with `git branch -d`: 38 `worktree-agent-*` refs and 37 merged
  `impl/*` refs. Their commits remain in `main`; only the stale pointers were removed.

### The worktree that appeared during the audit

After that first prune, a new registered checkout appeared at
`/home/timo/projects/flux-connectors-release-049`, branch `release/flux-0.49`. It carried C-455, the
owner-directed compatibility bump needed before flux-exchange can move. This was live work, not stale
debris, and was integrated in its own commit:

- the failing-first tree changed only `ENGINE_LINE` and filed C-455;
- commit `6e421af` moves all six requirements and eleven resolved packages to 0.49.0, records the
  source comparison, and adds both changelog entries;
- a later dirty snapshot was the transactional `0.13.0` cut in progress: 188 changed files consisting
  of version fields, changelog promotion and generated artefacts whose hashes carry that version. It
  contained no further source or story work and was deliberately not copied — the final cut
  regenerates it after both stories and every gate are complete.

The original C-455 story and source change are therefore commits on `main`; the only rejected bytes
are an interrupted release transaction that would otherwise predate this audit commit.

After the concurrent process stopped, its replacement checkout at
`/home/timo/projects/flux-connectors-release-049b` was clean at `6e421af`, exactly the same commit as
`main`. `git worktree remove` removed it and `git branch -d release/flux-0.49` removed the now-merged
branch. The temporary salvage tag was then deleted: the canonical minor cut is the sole remaining
release path.

### Gate

- 2026-08-02 — `connector-cli build` wrote nothing and `connector-cli diff` reported
  `951 artifacts up to date (54 providers checked)`.
- 2026-08-02 — The Rust build, `cargo test --workspace --no-fail-fast`, clippy and formatting gates
  are green. The public site built and passed 42 tests; the host UI passed 15 tests.
- 2026-08-02 — `scripts/publish-crates-io.sh --dry-run` packaged and verified the four-crate closure
  without uploading it.
