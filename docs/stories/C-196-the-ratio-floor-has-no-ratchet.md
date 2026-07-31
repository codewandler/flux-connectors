---
id: C-196
title: "`RATIO_FLOOR_PERCENT` has no ratchet, so it can only drift"
pillar: Build
status: ready
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

- [ ] **Failing-first test:** with the constants as they ship, a test proves the ratio floor may sit
      arbitrarily far below the measurement. Name it.
- [ ] Decide between the two shapes and record the reason:
      - **a two-way ratchet**, mirroring `the_recorded_floor_is_the_measured_figure`, with a slack
        chosen and justified rather than copied; or
      - **derive the ratio from `COVERED_FLOOR`** and delete the constant, on the grounds that two
        numbers describing one measurement will always drift apart eventually.
      The second is smaller and removes the class rather than guarding it — prefer it unless the
      ratio genuinely guards something the count does not.
- [ ] Whichever lands, the *stated* design survives: one honest absence must not turn an unrelated
      provider story red, and a connector arriving with no shapes at all must still fail.
- [ ] The scoped gate is green and the build stays a fixed point.

## Progress

- (not started)

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
