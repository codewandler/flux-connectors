---
id: C-456
title: "The catalogue network gate builds its configuration once, not once per operation"
pillar: Build
status: done
priority: 0
areas: [build]
note: "release-gate finding 2026-08-02: three safety assertions are CPU-bound because every operation rebuilds configuration by projecting the whole catalogue"
---

# The catalogue network gate builds its configuration once

## Goal

Keep the whole-catalogue permission-subject and intent proof intact while removing its accidental
quadratic setup cost from every local and release gate.

## Acceptance

- [x] **Measured first:** a warm targeted run of `connector-pack/tests/network_gate.rs` records the
      pre-change wall time, and process inspection confirms the delay is CPU work rather than network
      I/O.
- [x] The shared test configuration is constructed once and cloned into projected operations; no
      production global or cache is introduced.
- [x] All four assertions still enumerate the shipped catalogue and retain their non-empty controls.
- [x] A warm targeted run is green and records its wall time against the same command.
- [x] The full release gate is green after the change.

## Progress

- 2026-08-02 — The release gate reported three tests still running after 60 seconds. `ps` measured the
  test binary at approximately 299% CPU while the stand-in transport in `http()` has no live client.
- 2026-08-02 — Source inspection found the multiplicative path: each catalogue entry calls
  `tool_for()`, which calls `configuration()`, which loops over every catalogue entry and projects it
  with `probe()` to discover endpoint variables. The assertions therefore repeat a whole-catalogue
  construction once per operation.
- 2026-08-02 — The pre-change targeted test binary completed in 191.31 seconds (198.54 seconds for
  the command including 7.21 seconds compilation; 595.74 user CPU-seconds). After caching the test's
  immutable `Configuration` with `OnceLock` and cloning the port per projection, the same four tests
  completed in 1.48 seconds (1.86 seconds for the command; 2.82 user CPU-seconds): 129 times faster by
  the test harness's own wall-time figures.
- 2026-08-02 — The full generated-artifact and Rust gates are green; the public site built and passed
  42 tests, the host UI passed 15, and all four publishable crates package and verify under the clean-
  tree dry run.

## Notes

- This is test-only setup. `connector-pack` production configuration remains explicitly bound by a
  host and carries no global or ambient default.
