---
id: C-196
title: "`RATIO_FLOOR_PERCENT` has no ratchet, so it can only drift"
pillar: Build
status: done
priority: 3
design:
epic: connectors-v1
areas: [connector-spec]
note: "found while raising COVERED_FLOOR for the 2026-07-31 wave: its sibling `RATIO_FLOOR_PERCENT` had drifted from the 'one point under' its own doc specifies to SIX points — room for a whole provider to land with no response shapes and not be caught. The absolute floor has a two-way ratchet; the ratio floor has none"
---

# `RATIO_FLOOR_PERCENT` has no ratchet, so it can only drift

## Goal

Give the ratio floor the same two-way ratchet the absolute floor has, or derive it from the absolute
floor so there is only one number to keep honest — so that the guard which notices *"a connector
arrived carrying no response shapes at all"* cannot silently stop noticing.

## What was measured

`crates/connector-spec/tests/response_schema_coverage.rs` carries two floors:

| constant | guards | ratchet |
|---|---|---|
| `COVERED_FLOOR` | the absolute count; deleting a schema fails | **two-way** — `the_recorded_floor_is_the_measured_figure` fails if coverage runs more than a tenth of the catalogue ahead |
| `RATIO_FLOOR_PERCENT` | the share; a connector arriving with nothing fails | **none** |

The ratio floor's own doc states its design precisely:

> the floor is set one point under the measurement deliberately: a single honest absence — one
> operation whose vendor documents no body — should not turn an unrelated provider story red on
> arrival, while a connector landing with nothing still does. **There is no room in one point for a
> whole provider.**

It was `82`, one point under `83` when coverage was 92 of 110. By 2026-07-31 coverage was **220 of
248 — 88.7%**, so the gap was **six points**, not one. At 248 operations six points is roughly
sixteen operations: comfortably a whole provider landing with nothing, which is exactly the arrival
the constant exists to catch.

Nothing failed. Nothing could — no test compares this constant to the measurement in either
direction. It was doing the archaeology its sibling's ratchet was built to prevent, and the sibling's
doc says so in as many words: *"`COVERED_FLOOR` could sit at its entry value of 29 forever while the
catalogue improved, and the 'current figure' in the header would quietly become archaeology."*

Raised by hand to `87` in the same commit that raised `COVERED_FLOOR` to `220` — which is the
mechanism that had just failed, applied again.

## Acceptance

- [x] **Failing-first test:** with the constants as they ship, a test proves the ratio floor may sit
      arbitrarily far below the measurement. Name it.
- [x] Decide between the two shapes and record the reason:
      - **a two-way ratchet**, mirroring `the_recorded_floor_is_the_measured_figure`, with a slack
        chosen and justified rather than copied; or
      - **derive the ratio from `COVERED_FLOOR`** and delete the constant, on the grounds that two
        numbers describing one measurement will always drift apart eventually.
      The second is smaller and removes the class rather than guarding it — prefer it unless the
      ratio genuinely guards something the count does not.
- [x] Whichever lands, the *stated* design survives: one honest absence must not turn an unrelated
      provider story red, and a connector arriving with no shapes at all must still fail.
- [x] The scoped gate is green and the build stays a fixed point.

## Progress

**Landed as a two-way ratchet on a new unit, not on the percentage.** `RATIO_FLOOR_PERCENT` is
deleted; `ABSENCE_CEILING = 33` (operations shipping *without* a response shape) and
`ABSENCE_SLACK = 2` replace it, with `the_recorded_ceiling_is_the_measured_absence` as the second
direction.

**Deriving from `COVERED_FLOOR` was weighed first, as the story asks, and rejected on evidence.**
`COVERED_FLOOR * 100 / operations` puts the same denominator on both sides of the comparison, so it
reduces to `covered >= COVERED_FLOOR` — the assertion immediately above it. The arrival the constant
exists to catch then passes: a nine-operation connector landing with nothing leaves `covered` at 268,
and 268 of 308 clears a derived floor of 81 easily. Derivation deletes the guard along with the
constant. The two constants bound quantities that move independently — a wave can raise covered and
absent in the same commit — and neither is computable from the other.

**Keeping a percentage with a ratchet bolted on was also rejected, and this is the finding worth
carrying forward.** At the merge base the measurement is 268 of 299 and the floor is 88, so *five*
operations could arrive carrying nothing before it fired — while **27 of the 53 shipped connectors
ship five operations or fewer**. No whole-percent value fixes that: one point of 299 operations is
three operations, so a percent cannot simultaneously admit one honest absence and refuse a
three-operation connector. The unit was the defect, not the value, which is why a ratchet alone would
not have restored the stated design. Counting absences directly makes the resolution one operation
and keeps it there as the catalogue grows.

**The slack is measured, not copied from `COVERED_FLOOR`'s tenth.** Bounded above by the smallest
shipped connector (supabase, 3 operations) so a connector that size arriving with nothing is caught;
bounded below by 1 so a single honest absence stays green. Of the two remaining values, 2 is what the
catalogue shows: datadog (2 of 4) and google (6 of 8) each arrived with exactly two honest absences,
and a slack of 1 would have turned both red for doing nothing wrong.

**The two arrivals, asserted rather than described.**
`a_connector_arriving_with_no_response_shapes_is_caught` holds both halves of the stated design
against the live measurement: one honest absence stays green, and a connector the size of the
smallest already shipped, arriving with nothing, goes red. It is the failing-first test — at the
merge base it fails on the second half.

**`AGENTS.md` needed one change and got it**: §"A ninth and tenth staleness check exist" now names
the new constant, its ownership, and the one behavioural change for provider implementors — a story
landing **three or more** honest absences is red on arrival and reports it rather than editing the
constant. Zero, one or two absences are unaffected, which covers every provider story the catalogue
has seen except babelforce (0 of 9) and fly (4).

## Notes

- **This is a claim-integrity story, like [C-189](C-189-the-lockfile-is-never-written.md) and
  [C-81](C-81-declared-counts-are-checked.md).** All three are the same shape: a number in this
  repository that describes reality, with nothing holding it to reality. C-81 is the largest
  instance and remains `ready`; the fact that this was found *while hand-correcting numbers for
  C-81's exact reason* is the argument for doing them together.
- **Do not simply raise the constant and close this.** That has now happened twice and is what
  produced the drift.
- Worth checking while here: whether `COVERED_FLOOR`'s own slack of "a tenth of the catalogue" still
  makes sense at 248 operations. A tenth was 11 operations at 110 and is 24 now, so the allowance
  for a provider story to land without touching the file has silently more than doubled.
