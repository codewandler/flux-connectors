---
id: C-105
title: "Provider fleet 2 — the next connectors, shipped in parallel (epic)"
pillar: Spec
status: ready
priority: 3
design:
epic: provider-fleet-2
areas: [providers, connector-spec, connector-flux]
note: "EPIC — the first fleet (C-69..C-78) is fully drained. Each connector here is chosen to exercise something the model has not yet met, not just to add a row"
---

# Provider fleet 2 — the next connectors, shipped in parallel (epic)

## Goal
Grow the catalogue past sixteen connectors, and do it a wave at a time rather than one at a time.

## Acceptance
- [ ] [C-104](C-104-parallel-provider-fanout.md) lands first — without it these stories conflict
      pairwise on one file and integrate serially however many run at once.
- [ ] Each connector below ships with the shape the first fleet established: a curated operation set
      (not every endpoint), declared risk and idempotency per operation, a `[[config]]` surface, a
      `verify` operation, and a per-provider contract test.
- [ ] **Each one earns its place by exercising something the model has not met.** A connector that
      only adds a row is a row; the first fleet's value was that Shopify forced the `header` scheme,
      Google forced services, and Asana forced a body envelope.
- [ ] The counts in `README.md` and `AGENTS.md` are refreshed once at the end of the wave, not per
      story — [C-81](C-81-declared-counts-are-checked.md) owns making that mechanical.

## Progress
- Not started. Filed 2026-07-30.

## Notes
- The first fleet is fully drained: C-69 Google, C-70 Jira, C-71 Asana, C-72 HubSpot, C-73 Intercom,
  C-74 Shopify, C-75 Airtable, C-76 OpenRouter, C-77 Sentry, C-78 Zoom — all `done`.
- **Vendor selection is the judgement most worth revisiting.** These five were chosen for what they
  force the model to confront; a different five would be equally defensible, and swapping one is
  cheap while they are still unstarted.
