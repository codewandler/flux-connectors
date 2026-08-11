---
id: C-533
title: "The test suite links one binary per crate, not one per file"
pillar: Build
status: ready
priority: 1
areas: [tests, connector-cli, connector-spec, connector-flux, connector-pack]
note: "179 integration test files are 179 separate crates, each statically linking the whole dependency graph — 30 GB of executables, and the disk exhaustion that failed a release cut"
---

# The test suite links one binary per crate, not one per file

## Goal

Stop paying 179 full static links to run the test suite, without deleting or merging a single test.

## The measurement

Every `.rs` file directly in a `tests/` directory is its own **crate**. Cargo compiles and links each
into a standalone binary carrying the entire dependency graph. Nobody chose 179 binaries; 179 files
were written, and that is what Cargo does with them.

Measured on 2026-08-12:

| | |
|---|---|
| Integration test files | **179** — 67 `connector-flux`, 54 `connector-spec`, 29 `connector-cli`, 23 `connector-pack`, 5 `connector-secrets`, 1 `catalog` |
| Executables in `target/debug/deps` | **792** (`find … -executable \| wc -l`) |
| Their total size | **30.0 GB**, mean **38.8 MB** (`find … -printf "%s\n" \| awk`) |
| Whole `target/` | **43.5 GiB** (`cargo clean` reported it) |
| Cores used by one full pipeline | **1 of 20**, 29.3s at 98% CPU (`time cargo run -q -p connector-cli -- diff`) |

**This is not only slow, it failed a release.** Cutting v0.21.0 with three agent worktrees present —
each with its own `target/` at 24 GB and 8.2 GB — exhausted an 848 GB disk. `rust-lld` died with
`ld terminated with signal 7 [Bus error]`, three `connector-pack` test binaries failed to compile,
the gate failed, and `cut-release.sh`'s own rollback then ran out of space mid-write and truncated
`Cargo.toml` to zero bytes. The tree was recoverable from git and nothing was lost, but the release
did not happen that evening.

## Acceptance

- [ ] Each crate's integration tests link **one** binary. The mechanism is the ordinary Rust one: a
      single `tests/main.rs` per crate declaring `mod <file>;` for each existing test file, with the
      files moved under `tests/main/` (or kept in place and reached with `#[path]`).
- [ ] **No test file is deleted, merged, renamed in substance, or has its assertions changed.** Each
      one is a documented argument in this repository, not a bag of assertions, and the module
      docs are the point. This story changes how they are *linked*, nothing else.
- [ ] The test count before and after is identical, proved by capturing the total from
      `cargo test --workspace -- --list` on both sides and diffing it. A test that stops being
      compiled is a test that stops failing, which is the one way this change could do harm silently.
- [ ] Shared support modules stop being `#[path]`-included into many crates and become ordinary
      modules of the one binary. `crates/connector-spec/tests/support/shipped_provider.rs` is the
      case that matters — it is currently `#[path]`-included across crates.
- [ ] The measured footprint is recorded in the story after the change, from the same commands, so
      the improvement is a measurement rather than a claim.
- [ ] The full gate passes, and `cargo run -q -p connector-cli -- diff` still reports a clean fixed
      point.

## Progress

- 2026-08-12: Filed after disk exhaustion failed the v0.21.0 cut. Measurements above are from that
  session.

## Notes

**Three separate costs, and this addresses all three.** Link time (179 links become 6), disk (30 GB
of executables becomes roughly one binary per crate), and wall clock — `cargo test` runs integration
test *binaries* one process at a time, so 179 binaries serialize against each other on a 20-core
machine.

**Two adjacent techniques, deliberately not folded in here.** `cargo-nextest` runs tests as parallel
processes and would attack the wall-clock half independently; parallelising
`crates/connector-cli/src/pipeline.rs:166` — a sequential `for provider in &providers` over 55
independent `compile()` calls — would cut the ~29s floor every test pays. Both are worth doing and
neither is this story, because this one is a pure build-graph change with no behavioural surface,
and mixing it with a parallelism change would make a red run ambiguous. The pipeline one in
particular must preserve byte-identical output from equal inputs, which this repository requires and
which deserves its own failing-first proof.

**What must not happen.** None of this may reduce what is verified. `cut-release.sh` runs the Rust
gate plus both Node gates, and the tempting version of "make the gate faster" is to run less of it.
Every one of the three costs above is addressable without dropping a single assertion.
