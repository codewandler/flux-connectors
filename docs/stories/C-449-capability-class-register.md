---
id: C-449
title: "Keep the candidate capability classes an evidence register, and re-measure it against the IR"
pillar: Spec
status: ready
priority: 3
design: docs/designs/capability-classes.md
epic: provider-roles
areas: [connector-spec, catalog]
note: "measured: the AI-shaped classes (agent-memory, vector-store, image-generation, transcription) have ZERO implementations and embeddings has one; `incident` has three vendors sharing a verb set and is the strongest candidate in the catalogue. Counts are upper bounds — catalog.json carries no `expose`"
---

# Keep the candidate capability classes an evidence register, and re-measure it against the IR

## Goal

Turn [capability-classes.md](../designs/capability-classes.md) from a one-off measurement into a
register that is **trustworthy** — re-measured against the IR rather than the catalogue, filtered to
the model-facing operation set, and with a stated rule for when a candidate graduates to a contract.

## Why this is a story and not just a document

The register was produced by regex over `web/public/catalog.json`, and its own §How this was measured
records two caveats that both cut toward **over-counting**:

1. `babelforce` compiles 391 operations and exposes 9, so it inflated four classes with operations no
   model can reach. It was excluded by hand.
2. **`catalog.json` carries no `expose` field**, so the remaining counts cannot be filtered to the
   model-facing set at all. Every number is an upper bound.

A register nobody can re-derive is exactly the "timestamped claim, not a fact" that `AGENTS.md`
warns about — and this one will be quoted in contract proposals, which is the most dangerous place
for a stale number.

## What the first pass found (2026-08-02, 54 providers, 679 operations)

- **Zero implementations** for `agent_memory`, `vector_store`, `image_generation`, and
  `transcription`/`speech_synthesis`. **One** for `embeddings` (`openai-embeddings-create`).
- **`incident` is the best-evidenced candidate in the catalogue**: `datadog`, `pagerduty` and
  `statuspage` each expose `get` and `list`, plus a lifecycle surface. Three vendors sharing a *verb
  set*, where the shipped `llm_catalogue` has three vendors sharing a single `list`.
- `search` is well-populated (4 vendors) and **carries balance-contract.md's failure mode**: the four
  search different kinds of object, so the call substitutes and the result does not.

## Acceptance

- [ ] The register's counts are **re-derived from the IR**, not from `catalog.json`, and filtered to
      `Operation::expose == true`. State the new numbers and which ones moved.
- [ ] The derivation is **reproducible**: a committed script, or a test, or an exact command in the
      document. A hand-run regex that nobody can repeat does not satisfy this.
- [ ] A stated **graduation rule** for when a candidate becomes a contract proposal. The starting
      point is C-121's — *"a mechanism validated by a single role is designed around a single case"* —
      i.e. at least two independent implementations. If the register adopts a different bar, it says
      why.
- [ ] Each candidate is classified by **mechanism, not just name**: `Role`, `tag`, a `verify`-shaped
      field, or nothing. The register already notes that `verify` is a universal single-slot contract
      declared by **28 providers** — nearly thirty times the adoption of the role mechanism — so a
      one-slot candidate is probably not a `Role`.
- [ ] Candidates with **zero implementations** are kept in a clearly separate "wanted" section and are
      never counted alongside populated ones.
- [ ] **No `Role` variant is added.** `connector-contracts.md` §Out of scope still refuses defining
      contracts ahead of the mechanism.
- [ ] `search`'s entry carries the incommensurable-results warning and cross-references
      [C-447](C-447-decide-balance-shape.md), and records that `zendesk-ticket-search` is currently
      non-functional per `AGENTS.md` §Intentional gaps.

## Progress
- (not started)

## Notes
- Filed under the existing `provider-roles` epic deliberately rather than as a new epic — that epic
  already owns the vocabulary question, and a third contracts epic would fragment it.
- Related: [C-121](C-121-llm-catalogue-role.md) proposes the `ticketing` role and is where `incident`
  should be argued if it graduates; [C-447](C-447-decide-balance-shape.md) and
  [C-448](C-448-a-contract-cannot-require-a-derived-value.md) are the balance half.
- If the re-measurement contradicts the first pass, say so plainly and correct the design doc — the
  first pass is a claim, not a fact.
