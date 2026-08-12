---
id: C-544
title: "Parallelise the provider compile loop"
pillar: Build
status: ready
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

- [ ] The per-provider compile loop in `crates/connector-cli/src/pipeline.rs` (measured at
      `pipeline.rs:166` on 2026-08-12; re-verify, the file has moved since) runs providers
      concurrently, and the wall-clock improvement for `cargo run -q -p connector-cli -- diff` is
      recorded in this story as a before/after measurement from the same command.
- [ ] Byte-determinism is proved, not assumed, with a failing-first test: a seeded ordering hazard
      (or an assertion over repeated builds) demonstrates the test can detect nondeterministic
      output, and the committed tree remains a fixed point — `diff` clean, `connectors.lock`
      byte-identical across repeated full builds, and the pack digest unchanged.
- [ ] Output ordering stays defined by content, never by completion order: artifact write order,
      diagnostic/refusal message order, and the lockfile row order are identical to the sequential
      build's. A refusal raised by one provider must surface identically regardless of scheduling.
- [ ] The offline guarantee and the refusal semantics are untouched: no new dependency opens a
      socket, and a provider's compile failure still fails the build loudly with the same message
      it fails with today.

## Progress

- 2026-08-12: Filed by the C-537/C-542 wave coordinator when the operator asked for faster test
  runs. C-533's notes name this technique as worth doing and deliberately out of that story's
  scope; it deserves its own failing-first determinism proof.

## Notes

- Write set: `crates/connector-cli/src/pipeline.rs` and its tests, possibly a concurrency
  dependency in `connector-cli`'s manifest (a lockfile/manifest change ⇒ runs solo). Collides with
  any connector-cli story; never share a wave with C-538 or C-543.
- The floor this removes is paid by every whole-tree fixed-point test, so the win multiplies
  across the suite rather than landing once.
