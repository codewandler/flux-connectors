---
id: C-404
title: "Enable the Graph → composite-operation lowering now that a response is a record"
pillar: Codegen
status: ready
priority: 2
note: "READY — C-403 landed the canonical {status, headers, body} response record and its host test; the lowering and its old refusal exist, so this story now removes the spent guard and proves a real graph"
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
- **2026-08-02 — unblocked.** C-403 is `done`; `docs/integrating-with-flux.md` and
  `live_egress::the_response_comes_back_as_a_record_not_a_flat_string` pin the record response.

## Notes
- C-403 was the prerequisite and is now done. Preserve the refusal's history when lifting it so the
  safety decision remains reviewable.
