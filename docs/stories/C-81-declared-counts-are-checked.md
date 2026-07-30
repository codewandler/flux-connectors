---
id: C-81
title: Make the documented provider and artifact counts a checked claim
pillar: Build
status: ready
priority: 5
design:
epic: connectors-v1
areas: [connector-cli, docs]
note: drifted five times in one session; every agent noticed and none could fix it
---

# Make the documented provider and artifact counts a checked claim

## Goal
Stop `AGENTS.md` and `README.md` stating provider, operation and artifact counts that nothing
verifies — the numbers drifted five times in a single session, and every implementor noticed while
none could safely fix them.

## Acceptance
- [ ] A test compares the counts stated in `AGENTS.md` and `README.md` against a real build:
      providers in `providers/`, curated operations, and planned artifacts. It fails when they
      disagree. `crates/connector-cli/tests/readme_snippet.rs` is the existing home for
      README-derived claims.
- [ ] The `diff` output line both documents quote (`N artifacts up to date (M providers checked)`) is
      checked as a string against the real command output, since that one is copy-pasteable and
      therefore the most likely to be trusted.
- [ ] Failing-first: bump a stated number, watch the test fail, restore it.
- [ ] The test names what to do when it fails — regenerate the number, do not relax the test — because
      the count legitimately changes with every provider.
- [ ] `AGENTS.md`'s existing caveat that the artifact count is "not a permanent invariant" is
      reconciled with the test: not permanent, but *checked at every commit*.

## Progress
- Not started. Filed 2026-07-30 after the fleet push.

## Notes
- **This is the same failure C-54 removed one layer down.** C-54 deleted five hand-maintained provider
  lists in test code because duplication silently dropped coverage; these are hand-maintained provider
  counts in prose, and they silently drop accuracy. The fix is the same shape: derive, do not restate.
- Every provider story in the fleet flagged the staleness in its report and correctly declined to
  touch the file, because four concurrent siblings each invalidated whatever number the others wrote.
  A coordinator fixing it by hand once per wave is not a fix; it is the same manual step that already
  failed five times.
