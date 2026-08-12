---
id: C-533
title: "The test suite links one binary per crate, not one per file"
pillar: Build
status: done
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

- [x] Each crate's integration tests link **one** binary. The mechanism is the ordinary Rust one: a
      single `tests/main.rs` per crate declaring `mod <file>;` for each existing test file, with the
      files moved under `tests/main/` (or kept in place and reached with `#[path]`). *(Delivered
      with `#[path = "main/<file>.rs"]` on every declaration — a bare `mod x;` in a crate root
      resolves in `tests/`, never `tests/main/`, so the attribute is load-bearing; each `main.rs`
      documents the rule.)*
- [x] **No test file is deleted, merged, renamed in substance, or has its assertions changed.** Each
      one is a documented argument in this repository, not a bag of assertions, and the module
      docs are the point. This story changes how they are *linked*, nothing else. *(All 201 files
      are git renames at 93–100% similarity; the non-mechanical deltas are include/import swaps
      plus two documented accommodations: `connectors-api`'s env lock unified into
      `crate::support::env_lock`, and `vault_live`'s self-spawn filter module-qualified.)*
- [x] The test count before and after is identical, proved by capturing the total from
      `cargo test --workspace -- --list` on both sides and diffing it. *(1892 = 1892; per-package
      normalized name multisets identical for all nine crates — name-level, not just counts.)*
- [x] Shared support modules stop being `#[path]`-included into many crates and become ordinary
      modules of the one binary. *(`shipped_provider` and `origin_corpus` are each declared once
      per consuming binary and reached as `use crate::…`; the 88 per-file `#[path]` includes are
      gone. One `#[path]` per consuming crate root remains for the cross-crate reach — removing
      that means a dev-dependency, deliberately not spent here.)*
- [x] The measured footprint is recorded in the story after the change, from the same commands, so
      the improvement is a measurement rather than a claim. **After (measured 2026-08-12, clean
      build, implementor's worktree):** integration link targets 201 → **9**; executables in
      `target/debug/deps` 792 → **38** (`find … -maxdepth 1 -type f -executable | wc -l`); their
      total size 30.0 GB → **1.1 GB** (mean 28.1 MB, n=38); whole `target/` 43.5 GiB → **2.9 GiB**
      (`du -sh`); full clean `cargo test --workspace --no-fail-fast` wall clock **473 s**.
      *Caveat: the story's before-figures were measured under full DWARF; the machine switched dev
      builds to `line-tables-only` + sccache + lld the same day, before the after-measurement, so
      the GB figures conflate this story with the profile change. The executable count (792 → 38)
      and link-target count (201 → 9) are profile-independent.* `diff` wall clock is unchanged
      (~30 s, 1 core) — pipeline parallelism is C-544's, not this story's.
- [x] The full gate passes, and `cargo run -q -p connector-cli -- diff` still reports a clean fixed
      point. *(1888 passed / 0 failed / 4 ignored = 1892; clippy `-D warnings` clean; `1167
      artifacts up to date (55 providers checked)`.)*

## Progress

- 2026-08-12: Filed after disk exhaustion failed the v0.21.0 cut. Measurements above are from that
  session.
- 2026-08-12: Implemented on `impl/C-533` (`0ea8aba1`), merged `108fd7a4`. The tree had grown to
  201 test files across nine crates by implementation time (the 179 was five crates on filing
  day); all nine crates converted, including the single-file ones, so the next test file added
  lands in the existing binary instead of minting a new executable. Test identity proven at name
  level (1892 = 1892, identical per-package multisets). The per-provider-scope enforcement was
  verified live on a seeded `read_dir(providers/)` violation at the new path. AGENTS.md's
  expected-reds table, single-test invocation advice and Validation prose were reconciled at
  integration.

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
