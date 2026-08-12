---
id: C-549
title: "One conformance test is the suite's wall-clock floor"
pillar: Build
status: ready
priority: 3
epic: test-suite-cost
areas: [connector-spec, tests]
note: "verification_conformance::every_shipped_hmac_scheme_is_covered_by_the_matrix runs 284-386 s against a 365-482 s total nextest run (measured 2026-08-12 by C-543) — one test is most of the remaining wall clock; split it without weakening what it proves"
---

# One conformance test is the suite's wall-clock floor

## Goal

The workspace suite's wall clock stops being dominated by a single test.
`verification_conformance::every_shipped_hmac_scheme_is_covered_by_the_matrix`
(`crates/connector-spec/tests/main/verification_conformance.rs`) measured 284–386 s under nextest
against a 365–482 s total run (C-543, 2026-08-12): with every other test parallelised, this one
test IS the floor. Split it so nextest can schedule its parts across cores — without weakening the
property it proves or turning a catalogue-wide claim into a hand-listed inventory (AGENTS.md's
per-provider-test-scope rules apply).

## Acceptance

- [ ] The conformance property survives intact: the same schemes, vectors and assertions run —
      proven by a before/after inventory of what is exercised (the C-533 count-identity pattern:
      compare the executed assertion surface, not just test names).
- [ ] The suite's critical path drops measurably: quote the slowest single test and the total
      nextest wall clock before and after, same machine, same commands.
- [ ] The split follows the repository's test-scope rules — a per-scheme or per-vector partition
      derived from the data, never a hand-maintained list a new scheme can silently miss; a scheme
      arriving with no partition must fail loudly, not skip.
- [ ] Whatever makes the test slow is named in the story (measured, not guessed) — if the cost is
      setup repeated per case, the split must not multiply it into a worse total.

## Progress

- 2026-08-12: Filed at C-543's integration from its measured adjacent finding.

## Notes

- Write set: `crates/connector-spec/tests/main/verification_conformance.rs` and possibly
  `crates/connector-spec/tests/main.rs` (module registration). Collides with any connector-spec
  story.
