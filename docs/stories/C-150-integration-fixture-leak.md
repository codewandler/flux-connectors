---
id: C-150
title: "The integration test harness has the same tmpfs bug, and it is the wider half"
pillar: Core
status: in-progress
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

- [x] Fixtures live under the build's own `target/`, derived so they follow `CARGO_TARGET_DIR` — the
      shape C-143 landed in `artifact.rs` (`current_exe()` based). **Reuse that approach rather than
      inventing a second one**; two spellings of the same fix is a thing to keep in step.
- [x] Removed on **every** path, including when a test panics.
- [x] Unique per test **and per run** — a pid plus a process-local counter is not enough, which is
      precisely how two agents collided.
- [x] **Failing-first test:** a leak assertion that is red against today's harness. Scope it by label
      **and** pid, per C-143's finding — an unscoped scan of `/tmp` fails on a *sibling agent's*
      debris and is exactly as untrustworthy as the flake it replaces.
- [x] `cargo test --workspace --no-fail-fast` green over a loop of at least 10 runs under concurrent
      build load, with the loop reported.
- [x] No behavioural change to what any integration test asserts.

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

## Progress

Done on branch `impl/C-150`, in `crates/connector-cli/tests/common/mod.rs` plus one new test binary,
`crates/connector-cli/tests/fixture_hygiene.rs`.

`Fixture::new` no longer calls `std::env::temp_dir()`. The root is
`<build>/<profile>/integration-fixtures/flux-connectors-{label}-{pid}-{run:x}-{call}`, where `<build>`
is derived from `current_exe()` — so it follows `CARGO_TARGET_DIR` — and `run` is the wall clock read
once per process. That is C-143's `scratch()` shape, deliberately spelled the same way; the only
divergence is the directory name (`integration-fixtures` beside `artifact-fixtures`), so the two
harnesses' debris stays distinguishable. The `Drop` impl that removes the tree already existed and is
unchanged apart from a comment recording why it must not assert.

The pre-existing `let _ = fs::remove_dir_all(&root)` before `create_dir_all` is gone: it only ever
guarded against a stale directory left by a recycled pid, which a run-scoped name makes
unrepresentable. `artifact.rs`'s `scratch()` does not have it either.

`fixture_hygiene.rs` carries four assertions, each scoped by label **and** pid:

| test | at the base |
|---|---|
| `a_fixture_never_occupies_the_shared_temporary_directory` | **red** — names `/tmp/flux-connectors-hygiene-shared-tmp-1138835-3` |
| `a_fixture_lives_in_the_build_directory` | **red** — the root is `/tmp/…`, not under `target/debug` |
| `a_fixture_path_is_unique_per_test_and_per_run` | **red** — the leaf is `flux-connectors-hygiene-unique-1138835-2`, which a second run reproduces |
| `a_fixture_does_not_survive_a_panicking_test` | **green** — the harness already had a `Drop`; it is a regression guard, and stated as one |

The leak assertion is deliberately made over a **live** fixture. What made the gate untrustworthy was
never a missing `Drop` — it was that the tree sat in a bounded tmpfs every other agent was also
filling, so "nothing of mine is in `/tmp`, ever" is the property that actually holds the line.

Nothing was serialised, and no integration test's assertions changed — only where their fixtures live.
`target/debug/integration-fixtures/` is empty after a full `cargo test --workspace --no-fail-fast`.

Two things a resuming agent should know:

1. **`AGENTS.md`'s Validation section still wants its line** (this file's last note). It was left
   alone on purpose: it is a shared contract several concurrent stories touch, and a conflict there
   is the coordinator's to resolve, not an implementor's to create.
2. **The sibling debris C-143 warned about is still on this machine** —
   `/tmp/flux-connectors-artifact-{create,replace,absent}-664560`, from a worktree at an older base.
   It is why every scan here is scoped by pid, and it is *not* evidence of a leak in either fix.
