---
id: C-543
title: "Run the test suite as parallel processes"
pillar: Build
status: done
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

- [x] The workspace gate runs tests through a parallel process runner (working choice:
      `cargo-nextest`; a competing option is rejected in writing, not silently), and the wall-clock
      improvement is recorded in this story as a before/after measurement from the same commands on
      the same machine. **Measured 2026-08-12 at `108fd7a4`, 20 cores, warm build:**
      `cargo test --workspace --no-fail-fast` 792.14 s (and its serialisation is total — the sum of
      its own per-target times is 790.63 s, 0.2 % apart); `cargo nextest run --workspace
      --no-fail-fast` 365.01 s + 2.06 s doc-tests = **2.16×** on the least-contended round; the
      honest floor under heavy shared load is ~1.6× (365→497 s as load rose 28→42, while cargo
      test barely moved, 792→794 s — it barely uses the machine). **Rejected in writing:** fanning
      `cargo test -p <crate>` per crate from the script or a CI matrix — no new tool, but it
      re-derives the member list as driftable inventory, interleaves nine binaries' output,
      hand-rolls `--no-fail-fast` as collect-and-resummarise, caps parallelism at 9 processes
      leaving the 553- and 535-test binaries on the critical path, and gives no per-test process
      isolation. Secondary: `taiki-e/install-action` declined to avoid a fourth third-party action
      in the merge-deciding job.
- [x] The verified surface does not shrink. **Counts reconcile exactly:** `cargo test --workspace
      -- --list` 1892 = nextest 1877 + 3 `#[ignore]` (visible under `--run-ignored all`) + 12
      doc-tests; `comm` in both directions shows nextest-not-cargo empty. Doc-tests stay as
      `cargo test --workspace --doc`.
- [x] `--no-fail-fast` semantics survive: proven in an out-of-tree scratch crate (3 failures
      across 3 binaries, all reported, one consolidated list) — and nextest's default is already
      no-fail-fast, with `.config/nextest.toml` pinning `fail-fast = false` so narrowed runs
      behave identically.
- [x] Every place the gate is spelled is updated together and stays consistent: AGENTS.md
      §Validation and the scoped provider gate, README's Contributing block (both applied by the
      coordinator at integration), `.github/workflows/ci.yml`, and `scripts/cut-release.sh` — whose
      stub-cargo test fixture learned `nextest` and now pins the honest spelling instead of
      surviving by prefix accident.
- [x] The runner is pinned (0.9.143, recorded in `.config/nextest.toml` which refuses a stale
      runner loudly), and a machine without it fails loudly with the install command: the
      cut-release preflight moved to step 2 — before anything is touched, per the script's own
      "a refusal costs nothing" heading — and uses `command -v cargo-nextest` so the test stub
      cannot intercept the answer.

## Progress

- 2026-08-12: Filed by the C-537/C-542 wave coordinator when the operator asked for faster test
  runs. C-533's notes name this technique as worth doing and deliberately out of that story's
  scope, because C-533 is a pure build-graph change and mixing the two would make a red run
  ambiguous.
- 2026-08-12: Implemented on `impl/C-543` (`f4940153`, `a7abb650`), merged `867a0329`, after one
  rework round: converting cut-release.sh's gate required its stub-cargo test fixture to learn
  `nextest`, which sat behind the wave's fence — the implementor proved the collision (7/11 red),
  stopped, and finished atomically once the fence was lifted for exactly two files. The
  connector-secrets CI matrix jobs deliberately stay on plain `cargo test`: per-process
  parallelism buys nothing for one crate, and rewriting the root-privileged `--ignored --exact`
  invocations risks turning the ownership proof into a silent skip. The suite's wall-clock floor
  is now the single `verification_conformance::every_shipped_hmac_scheme_is_covered_by_the_matrix`
  test (284–386 s) — filed as C-549.

## Notes

- Write set: AGENTS.md, `.github/workflows/ci.yml`, `scripts/cut-release.sh`, and possibly a
  `.config/nextest.toml`. Collides with any story amending the gate or the release script; do not
  share a wave with C-533 (it changes what the test binaries *are*) — land C-533 first so the
  measurement here is taken against the final binary layout.
- Independent of C-533 in mechanism, dependent in measurement: nextest parallelises across
  binaries, so its relative win is largest *before* C-533 collapses 179 binaries into ~6 — but the
  ambiguity rule above still orders C-533 first.
