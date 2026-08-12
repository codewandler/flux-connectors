---
id: C-543
title: "Run the test suite as parallel processes"
pillar: Build
status: ready
priority: 2
epic: test-suite-cost
areas: [tests, release]
note: "cargo test runs integration-test binaries one process at a time on a 20-core machine; cargo-nextest attacks the wall-clock half independently of C-533's link-count half — named as adjacent-but-separate in C-533's notes"
---

# Run the test suite as parallel processes

## Goal

Stop running the workspace's integration-test binaries one process at a time on a 20-core machine,
without reducing what is verified: the gate keeps every test, every expected-failure surface, and
doc-test coverage.

## Acceptance

- [ ] The workspace gate runs tests through a parallel process runner (working choice:
      `cargo-nextest`; a competing option is rejected in writing, not silently), and the wall-clock
      improvement is recorded in this story as a before/after measurement from the same commands on
      the same machine.
- [ ] The verified surface does not shrink. The runner's test count is diffed against
      `cargo test --workspace -- --list` before adoption, and doc-tests — which nextest does not
      run — stay in the gate explicitly (`cargo test --workspace --doc` or equivalent).
- [ ] `--no-fail-fast` semantics survive: a full run reports every failure, not the first one, and
      AGENTS.md's "expected staleness reds" workflow for provider stories still produces a complete
      named list in one run.
- [ ] Every place the gate is spelled is updated together and stays consistent: AGENTS.md
      §Validation, `.github/workflows/ci.yml`, and `scripts/cut-release.sh` — the transactional
      release gate must invoke the same runner the documentation names.
- [ ] The runner is pinned (version recorded), and a machine without it installed fails loudly with
      the install command, never by silently falling back to running less.

## Progress

- 2026-08-12: Filed by the C-537/C-542 wave coordinator when the operator asked for faster test
  runs. C-533's notes name this technique as worth doing and deliberately out of that story's
  scope, because C-533 is a pure build-graph change and mixing the two would make a red run
  ambiguous.

## Notes

- Write set: AGENTS.md, `.github/workflows/ci.yml`, `scripts/cut-release.sh`, and possibly a
  `.config/nextest.toml`. Collides with any story amending the gate or the release script; do not
  share a wave with C-533 (it changes what the test binaries *are*) — land C-533 first so the
  measurement here is taken against the final binary layout.
- Independent of C-533 in mechanism, dependent in measurement: nextest parallelises across
  binaries, so its relative win is largest *before* C-533 collapses 179 binaries into ~6 — but the
  ambiguity rule above still orders C-533 first.
