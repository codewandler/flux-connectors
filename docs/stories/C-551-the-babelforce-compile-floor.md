---
id: C-551
title: "One provider is a third of the whole compile"
pillar: Build
status: ready
priority: 3
epic: test-suite-cost
areas: [connector-cli, connector-spec]
note: "taskset-measured by C-544: babelforce compiles in 10.3 s of the ~31 s sequential catalogue — five vendored OpenAPI documents loaded and lowered serially. Amdahl caps C-544's axis at ~3x until this drops; every fixed-point test and every diff pays it"
---

# One provider is a third of the whole compile

## Goal

babelforce's compile — five vendored OpenAPI documents, loaded and lowered serially — is 10.3 s of
the catalogue's ~31 s sequential cost (measured 2026-08-12 under `taskset -c 0`, C-544's handoff).
With providers now compiled concurrently, this single provider is the critical path of every full
`build`, `diff`, and whole-tree fixed-point test. Cut the floor by parallelising or otherwise
cheapening the intra-provider work, without touching determinism: byte-identical output, identical
refusal messages and ordering.

## Acceptance

- [ ] What the 10.3 s is actually spent on is measured first (parse vs. schema validation vs.
      lowering vs. rendering, per document) and recorded here — the fix follows the measurement,
      not the guess.
- [ ] The full-catalogue compile floor drops measurably (`taskset -c 0` per-provider before/after
      plus the parallel total, same machine, quoted), and C-544's width-equality and
      refusal-ordering tests stay green untouched.
- [ ] The committed tree remains a fixed point: no artifact byte moves, `connectors.lock` and the
      pack digests unchanged across repeated builds.

## Progress

- 2026-08-12: Filed at C-544's integration from its measured adjacent finding.

## Notes

- Write set: likely `crates/connector-spec` ingest and/or `crates/connector-cli` compile path —
  scope after the measurement. Collides with C-549/C-550 only on connector-spec/cli test files;
  the measurement half is read-only and can start any time.
