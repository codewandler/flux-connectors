---
id: C-550
title: "The plan's worker width is blind to a process-parallel runner"
pillar: Build
status: ready
priority: 3
epic: test-suite-cost
areas: [connector-cli, tests]
note: "C-543 runs each test as its own process across every core; C-544 gives each full-catalogue plan() 20 workers. ~5 whole-tree fixed-point tests x 20 threads oversubscribe a 20-core box — correctness unaffected, but the two speedups may not compose linearly (C-544's measured adjacent finding)"
---

# The plan's worker width is blind to a process-parallel runner

## Goal

C-543 (nextest, one process per test across every core) and C-544 (20 compile workers per full
`plan()`) compound into oversubscription: several whole-tree fixed-point tests each spawning a
full worker pool on an already-saturated machine. Correctness is unaffected — the pool is bounded
per call and joins before returning — but the two speedups may not add. Decide the width policy
under a nested runner (an environment override, a saturation heuristic, or measured evidence that
the oversubscription is harmless), and record the measurement either way.

## Acceptance

- [ ] The interaction is measured, not guessed: total nextest wall clock and the slowest
      fixed-point tests' times at the current default versus at least one bounded-width
      alternative, same machine, quoted.
- [ ] Whatever policy wins is deterministic in output (C-544's width-equality tests already pin
      that any width produces the identical Plan — they must stay green untouched) and documented
      where the width is chosen.
- [ ] If the answer is "leave it — measured harmless", that is a legitimate close: record the
      numbers in this story and change nothing.

## Progress

- 2026-08-12: Filed at C-544's integration from its adjacent finding.

## Notes

- Write set: `crates/connector-cli/src/pipeline.rs` (width choice only) and possibly
  `.config/nextest.toml`. Collides with any connector-cli story.
