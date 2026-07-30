---
id: C-150
title: "The integration test harness has the same tmpfs bug, and it is the wider half"
pillar: Core
status: ready
priority: 2
areas: [connector-cli]
note: "tests/common/mod.rs:58 builds its fixture root from env::temp_dir() — the identical bug C-143 fixed in artifact.rs, but in the harness EVERY integration binary uses. Two agents independently reproduced it taking down wiring, no_network, service_units and site_catalog"
---

# The integration test harness has the same tmpfs bug, and it is the wider half

## Goal

Fix `crates/connector-cli/tests/common/mod.rs` the way [C-143](C-143-artifact-tests-leak-fixtures.md)
fixed `artifact.rs`: fixtures in the per-worktree build tree, removed on every path including a panic,
under a name no run repeats.

## Why this is the more important half

C-143 fixed three unit tests. This is the harness **every integration binary** uses, so it is far more
exposed — and **two agents reproduced it independently, in the same wave, without coordinating**:

- One measured that with the tmpfs exhausted, `fs::write(...).expect("write fixture file")` at
  `tests/common/mod.rs:84` fails and takes down **`wiring`, `no_network`, `service_units` and
  `site_catalog`** — five of the eight red tests in that run, and output that reads exactly like a
  merge regression.
- The other saw `no_network` and `service_units` flake with **7 tests** panicking at the same line,
  green unchanged on re-run, against fixtures that could not be reached by its diff at all.

`Fixture::new` (`:58`) builds its root from `std::env::temp_dir()` as
`flux-connectors-{label}-{pid}-{counter}`. `/tmp` here is a **32 GB tmpfs**; a pid plus a
process-local counter does not separate two agents running the same binary.

**This is what made the gate untrustworthy twice.** Both times the first hypothesis was "the merge
broke it" — once against C-114, once against C-95, where a good merge was reverted before the cause
was measured. A flaky integration gate is worse than a missing one, because it teaches a coordinator
to distrust a red gate, which is the one signal that has to stay trustworthy.

## Acceptance

- [ ] Fixtures live under the build's own `target/`, derived so they follow `CARGO_TARGET_DIR` — the
      shape C-143 landed in `artifact.rs` (`current_exe()` based). **Reuse that approach rather than
      inventing a second one**; two spellings of the same fix is a thing to keep in step.
- [ ] Removed on **every** path, including when a test panics.
- [ ] Unique per test **and per run** — a pid plus a process-local counter is not enough, which is
      precisely how two agents collided.
- [ ] **Failing-first test:** a leak assertion that is red against today's harness. Scope it by label
      **and** pid, per C-143's finding — an unscoped scan of `/tmp` fails on a *sibling agent's*
      debris and is exactly as untrustworthy as the flake it replaces.
- [ ] `cargo test --workspace --no-fail-fast` green over a loop of at least 10 runs under concurrent
      build load, with the loop reported.
- [ ] No behavioural change to what any integration test asserts.

## Notes

- **Do not serialise anything.** That hides the cleanup bug rather than fixing it.
- **The load generator must target the real disk, not the scratchpad.** C-143's implementor learned
  this the hard way: its first loop wrote a target dir onto the `/tmp` tmpfs, filled 32 GB within
  minutes, and produced 10/10 runs failing with ~24 red tests across five binaries — **none of them
  its own**. It also broke output capture, since that needs `/tmp` too. If a flake hunt starts
  producing failures everywhere, suspect the harness before the diff.
- Also from that story: `pkill -f <pattern>` is dangerous here, because the pattern can match the
  agent harness's own command line.
- After this lands, `AGENTS.md`'s Validation section is worth a line: a red integration binary whose
  panic is inside `tests/common/mod.rs` is an environment failure, not a regression.
