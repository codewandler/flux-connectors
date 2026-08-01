---
id: C-445
title: "Inventory the Managed Agents surface before any TOML is written"
pillar: Spec
status: ready
priority: 2
design: docs/designs/anthropic-managed-agents.md
epic: anthropic-managed-agents
areas: [providers]
note: "C-130's lesson applied up front — that inventory contradicted its own epic's premise and the TOML was never written. No vendored document exists here, so this is hand-authored and C-126's do-not-invent rule binds hardest"
---

# Inventory the Managed Agents surface before any TOML is written

## Goal

Produce the written inventory the connector would be built from: which endpoints exist, which belong
in a curated set, how they partition into services, what each one's risk and idempotency actually
are, and — for every endpoint **not** carried — the reason.

## Why an inventory story at all

[C-130](C-130-ivr-atomics-inventory.md) is the precedent and the warning. Its inventory was written
from the source rather than from the epic's assumptions, and it **contradicted the premise**: five
independent findings, no TOML written, and a fence test shipped instead. That was the right outcome
and it was only reachable because the inventory came first. This epic has the same risk profile —
a large surface, no vendored document, and an assumed shape.

## The constraint that dominates: nothing is vendored

`specs/` holds `anthropic/2023-06-01-excerpt.yaml` (3.6 KB — no Admin API, no Managed Agents),
`babelforce/`, `flux/`, `zendesk/`. Builds are hermetic and offline, so there is no spec route here:
every operation is hand-authored, as `anthropic`'s `admin` service already is.

**This makes [C-126](C-126-response-schema-coverage.md)'s rule the sharpest constraint in the epic.**
A response field not known with confidence is left out, or left untyped with a note saying so — never
guessed into a `required` list. An invented schema that looks derived is worse than no schema. Where
the reference is silent, the inventory says "unknown", and that is a finding rather than a gap to
fill.

## Acceptance

- [ ] Every Managed Agents endpoint is listed with method, path, and a one-line purpose, sourced from
      the bundled `claude-api` skill reference (`shared/managed-agents-api-reference.md`) — invoke the
      skill; do not answer from memory.
- [ ] Each endpoint is marked **carry / withhold**, and **every withheld one carries its reason**.
      `providers/babelforce.toml` is the model for this three-category accounting (emitted /
      inexpressible / withheld).
- [ ] A proposed **service partition** with its rationale — one service or several (`agents`,
      `sessions`, `environments`, `vaults`, `memory`). The service is load-bearing: it owns
      `base_url` and `api_version`, and it keys credential addressing. Note the whole surface is
      beta-gated by `anthropic-beta: managed-agents-2026-04-01`, which is a `const_headers` case
      (`ir.rs:328`), not a parameter.
- [ ] **`archive` is terminal on agents, environments and memory stores — no unarchive.** Every such
      operation's `risk` reflects that, and the inventory says so explicitly rather than leaving a
      reader to infer it from `destructive`.
- [ ] Session-plane endpoints are inventoried but flagged as **gated on
      [C-444](C-444-decide-managed-agents-charter.md)**; the inventory does not pre-empt that decision.
- [ ] Pagination is recorded: this surface uses `page`/`next_page`, unlike the `after_id`/`before_id`
      scheme elsewhere in Anthropic's API. Note `quirks.pagination` reaches no artifact today.
- [ ] Anything the reference does not state with confidence is recorded as **unknown**, not filled in.
- [ ] **No `providers/anthropic.toml` edit in this story.** The inventory is a document; the TOML is
      C-446, and C-441 holds that file until it integrates.

## Progress
- (not started)

## Notes
- Write the inventory into `docs/designs/anthropic-managed-agents.md` or a sibling doc; do not put it
  in the story body.
- Managed Agents is not a Bedrock/Vertex/Foundry surface — it is Claude API (and Claude Platform on
  AWS) only. If the connector's `base_url` implies otherwise, say so.
