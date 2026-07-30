---
id: C-143
title: "The artifact tests leak their fixtures and go flaky under load"
pillar: Core
status: ready
priority: 3
areas: [connector-cli]
note: "found twice during a 7-agent wave, both times attributed to the wrong diff before being measured. 55 stale fixture directories in /tmp, which is a 32G tmpfs — the tests write to env::temp_dir() and do not always clean up"
---

# The artifact tests leak their fixtures and go flaky under load

## Goal

Make `crates/connector-cli/src/artifact.rs`'s tests deterministic under concurrency, and stop them
leaving directories behind.

## What was measured

Three tests — `write_atomic_creates_missing_directories`,
`write_atomic_replaces_and_leaves_no_temporary`, `read_if_exists_distinguishes_absent_from_present` —
failed a full `cargo test --workspace` twice during a wave with seven concurrent agents, and **passed
in isolation and on immediate re-run both times**.

The cause is environmental, not in the diff that happened to be merging:

- `scratch()` (`crates/connector-cli/src/artifact.rs:78`) builds a path under
  `std::env::temp_dir()`, keyed on a label and the process id.
- `/tmp` on this machine is a **32 GB tmpfs**, measured at **80% full**.
- **55** `flux-connectors-artifact-*` directories were sitting there, so the fixtures are not reliably
  removed.

Under a wave of concurrent cargo builds, tmpfs pressure is enough to fail a write.

## Acceptance

- [ ] The fixtures are removed on **every** path, including when a test panics — a guard type with a
      `Drop` impl, or a crate that already does this. A leaked directory is the observable symptom.
- [ ] **Failing-first test:** an assertion that no `flux-connectors-artifact-*` directory survives the
      suite. It must fail against today's code, which leaks.
- [ ] The tests do not depend on a shared, unbounded location. Prefer a scratch directory under the
      build's own `target/`, which is per-worktree and already isolated, over `env::temp_dir()`, which
      is shared by every concurrent agent on the machine.
- [ ] The path is unique per test **and** per run — a process id alone is reusable, and two agents
      running the same binary can collide.
- [ ] `cargo test --workspace --no-fail-fast` is green in a loop of at least 10 runs while something
      else is building.

## Notes

- **This cost real time twice**, and both times the first hypothesis was "the merge broke it" — once
  against C-114, once against C-95. A flaky test in the integration gate is worse than a missing one,
  because it teaches a coordinator to distrust a red gate, which is the one signal that must stay
  trustworthy.
- The right fix is probably to stop using `env::temp_dir()` at all. `target/` is per-worktree, already
  ignored, and cannot be contended by another agent's run.
- Do not "fix" this by serialising the tests. Serialising hides a cleanup bug that will resurface
  wherever else the fixtures are used.
- Related but separate: `/tmp` filling up is a machine-level condition that also surfaces as opaque
  compiler and linker errors during a large wave. `AGENTS.md` warns about disk as a running budget;
  this story is about the tests' own hygiene, not about the machine.
