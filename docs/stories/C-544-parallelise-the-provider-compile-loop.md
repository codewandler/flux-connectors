---
id: C-544
title: "Parallelise the provider compile loop"
pillar: Build
status: done
priority: 2
epic: test-suite-cost
areas: [connector-cli]
note: "pipeline.rs runs 55 independent provider compile() calls sequentially — 29.3s at 1 of 20 cores (C-533's measurement); every full build, diff, and fixed-point test pays that floor"
---

# Parallelise the provider compile loop

## Goal

Cut the ~29-second single-core floor that every full `build`, `diff`, and whole-tree fixed-point
test pays, by compiling the 55 independent providers concurrently — while provably preserving
byte-identical output from equal inputs, which this repository's determinism contract requires.

## Acceptance

- [x] The per-provider compile loop in `crates/connector-cli/src/pipeline.rs` (measured at
      `pipeline.rs:166` on 2026-08-12; re-verify, the file has moved since — it was at `:177` at
      the implementation base after C-537) runs providers concurrently, and the wall-clock
      improvement is recorded here. **Measured 2026-08-12 at `108fd7a4`, 20 cores, interleaved A/B
      with real binaries from base and HEAD at low load:** base 31.1–32.0 s, after 12.7–13.5 s —
      **~2.6×**, at +3–8 % total CPU. The scaling curve under `taskset` (1 core 31.6 s, 2 → 19.2,
      4 → 14.0, 20 → 12.2) shows the floor: babelforce alone is 10.3 s of the ~31 s sequential
      run, so Amdahl caps this axis at ~3× — going lower is intra-provider work, filed as C-551.
- [x] Byte-determinism is proved, not assumed, with a failing-first test: the seeded
      completion-order fold turns three of the four unit tests red — including the concrete
      wrong-provider-refusal failure — and
      `pipeline::tests::folding_in_completion_order_would_move_a_published_artifact` stays in the
      suite permanently as the tripwire. The committed tree stays a fixed point: `diff` clean at
      widths 1, 2, 4, 8 and 20; `connectors.lock` and the pack byte-identical across repeated
      full builds (digests quoted in the implementor's handoff).
- [x] Output ordering stays defined by content, never by completion order:
      `the_plan_is_identical_at_every_compile_width` compares the full Plan — paths, contents,
      change verdicts, diagnostics, orphans — at width 1 (which spawns no thread and IS the
      pre-C-544 loop) against widths 2, 3, 6 and 64;
      `a_refusal_is_the_first_one_in_provider_order_at_every_width` pins the refusal to the first
      in provider order, byte-identical at every width.
- [x] The offline guarantee and the refusal semantics are untouched: std::thread::scope only —
      Cargo.toml and Cargo.lock untouched; `no_network` and `dependency_fence` green; a worker
      panic is re-raised via `resume_unwind` so it surfaces exactly as an inline panic did.

## Progress

- 2026-08-12: Filed by the C-537/C-542 wave coordinator when the operator asked for faster test
  runs. C-533's notes name this technique as worth doing and deliberately out of that story's
  scope; it deserves its own failing-first determinism proof.
- 2026-08-12: Implemented on `impl/C-544` (`415506d5`), merged `65e2f822`. Concurrency-safety was
  audited, not assumed: the compile path's only shared state is three `OnceLock`s and flux_lang's
  per-thread RAII guards, none of which reach an output ordering. Two named risks carried
  forward: peak RSS roughly doubles (~21 MB of vendored specs resident for up to 20 providers at
  once), and workers run on 2 MiB default stacks — flux_lang's `MAX_LOWER_DEPTH` is sized for
  that, but a deep-lowering overflow would look here first. The interaction with C-543 (nextest
  processes × 20-worker plans oversubscribing a 20-core box) is filed as C-550; the babelforce
  intra-provider floor as C-551.

## Notes

- Write set: `crates/connector-cli/src/pipeline.rs` and its tests, possibly a concurrency
  dependency in `connector-cli`'s manifest (a lockfile/manifest change ⇒ runs solo). Collides with
  any connector-cli story; never share a wave with C-538 or C-543.
- The floor this removes is paid by every whole-tree fixed-point test, so the win multiplies
  across the suite rather than landing once.
