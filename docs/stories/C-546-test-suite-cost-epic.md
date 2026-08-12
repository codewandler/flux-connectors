---
id: C-546
title: "The test suite costs what it verifies, not what Cargo defaults to (epic)"
pillar: Build
status: in-progress
priority: 1
epic: test-suite-cost
areas: [tests, connector-cli, release]
note: "Epic tracker for the three measured test-cost levers: one binary per crate (C-533), parallel process running (C-543), and the parallel provider compile loop (C-544). Filed when the operator asked for faster test runs; C-533's measurement section is the evidence base"
---

# The test suite costs what it verifies, not what Cargo defaults to (epic)

## Goal

The workspace gate verifies everything it verifies today at a fraction of the wall clock, link time
and disk it pays now — without deleting, merging, or weakening a single test. Three independent,
measured levers, each its own story because mixing them makes a red run ambiguous:

| lever | story | attacks |
|---|---|---|
| one test binary per crate, not per file | C-533 | link count (~200 → ~6), 30 GB of executables, serial binary execution |
| a parallel process runner for the gate | C-543 | wall clock across binaries; keeps doc-tests and `--no-fail-fast` semantics |
| concurrent provider compiles in the pipeline | C-544 | the ~29 s single-core floor every full build, `diff` and fixed-point test pays |

## Acceptance

- [ ] C-533, C-543 and C-544 are done, in that order (C-533 changes what the binaries are; C-543
      measures against the final layout; C-544 is independent but shares `connector-cli`).
- [ ] Each story's before/after measurement is recorded in that story from the same commands, so
      the epic's total is a sum of measurements rather than a claim.
- [ ] Nothing verified shrank: test-count identity (C-533), doc-test coverage and complete-failure
      reporting (C-543), and byte-identical build output (C-544) are each proved in their story.

## Progress

- 2026-08-12: Filed as the epic over C-533 (already in flight when filed) and the C-543/C-544
  pair, when the operator asked for faster test runs during the v0.22.0/v0.23.0 sessions. The
  machine-level levers that needed no story — `debug = "line-tables-only"`, `sccache`, `lld` —
  are already wired in `~/.cargo/config.toml` with their reasoning recorded there.

## Notes

- Epic trackers are never dispatched; the children are.
- The evidence base is C-533's "The measurement" section (2026-08-12): 179 integration test files,
  792 executables at 30.0 GB in one `target/debug/deps`, and the v0.21.0 release cut that disk
  exhaustion failed.
