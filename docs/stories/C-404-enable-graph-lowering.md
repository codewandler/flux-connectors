---
id: C-404
title: "Enable the Graph → composite-operation lowering now that a response is a record"
pillar: Codegen
status: blocked
priority: 2
note: "the lowering is BUILT and has zero providers using it, because a composite op could not read a field out of a previous step's flat-string response. flux 0.43 made the response a record; C-403 is what brings that here"
---

# Enable the Graph → composite-operation lowering

## Goal

Make composed operations real: a `graphs` entry in a provider definition lowering to a Flux composite
operation that reads a field out of a previous step's response.

## Why now

The machinery already exists — `Graph` is a provider-TOML field, the lowering is implemented,
`confirm` is included — and **zero providers use it**, because a composite operation could not select
`$resp.body.data.id` out of a flat string. flux 0.43 changed `http.request` to return
`{status, headers, body}`; C-403 brings that line here.

This is the prerequisite for two things beyond this repository: the aggregated intent-writes a
governed control-plane surface needs, and flux-exchange's promise of custom operations that compile
to the same Flux as vendor ones and are indistinguishable from them to a caller.

## Acceptance

- [ ] The refusal that currently fires for a composite reading a previous response is lifted, and the
      reason it existed is recorded as closed rather than deleted.
- [ ] **Failing-first test** — a two-step graph where the second step reads a field from the first
      step's response body, emitted and analyzed, failing before the change.
- [ ] At least one shipped provider declares a graph, so the path has a real user rather than only a
      test. Choose one where the composition is genuinely useful, not one contrived to exercise it.
- [ ] `confirm` semantics inside a graph are asserted, not assumed.
- [ ] Risk derivation for a composite is stated: a composite is at least as risky as its riskiest
      member, or the rule is written down if it is something else.

## Progress
- (blocked on C-403)

## Notes
- Do not start before C-403. The lowering's refusal is *correct* for the pinned flux, and lifting it
  first would ship a composite that cannot run.
