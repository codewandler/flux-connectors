---
id: C-140
title: "Search that is good enough to act on"
pillar: Bridge
status: ready
priority: 4
design: docs/designs/connectors-datasource.md
epic: connectors-datasource
areas: [bridge]
note: "a search that returns the wrong connector confidently is worse than no search, because the caller acts on it. Role-aware search is what makes the roles epic pay off — 'find me a ticketing provider' is a role query"
---

# Search that is good enough to act on

## Goal

Make `search` over the connectors catalogue return the right thing, in a stable order, with a reason
a caller can check.

## Acceptance

- [ ] Search covers the fields a caller actually reasons in: vendor, description, operation id, host,
      and — once [C-119](C-119-provider-roles-epic.md) lands — **role**.
- [ ] **Ranking is deterministic.** A tie broken by hash iteration order makes the same question
      answer differently between builds; the catalogue's fixed-point discipline should extend to its
      queries. **Failing-first test:** `the_same_query_ranks_identically_across_runs`, run over a
      shuffled input order.
- [ ] Every `Match` carries **why it matched** — which field, and on what term. A match a caller
      cannot explain is a match a caller cannot check, and this surface exists to be consumed by a
      model that will otherwise invent a justification.
- [ ] A query matching nothing returns an **empty result with the query echoed**, never a
      best-effort nearest hit. Confidently wrong is the failure mode that matters here.
- [ ] A test asserts a role query returns exactly the providers holding that role — the query that
      makes the roles epic pay off, and the one most likely to be silently wrong.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **Depends on [C-139](C-139-datasource-backend.md)** for the backend, and is much more useful after
  [C-119](C-119-provider-roles-epic.md)/[C-121](C-121-llm-catalogue-role.md) give roles to search on.
  It can land before roles — just say so, and do not pretend the role dimension exists.
- **Deterministic lexical search first.** `flux-capabilities` has an `Embedder` and semantic search
  may be worth it later; ship the simple thing and find out whether it is actually insufficient
  rather than assuming. An embedding index also has to be built, stored and kept in step with the
  catalogue, which is real cost against an unmeasured benefit.
- Do not let ranking quietly encode a preference between vendors. If two connectors genuinely fit a
  query equally, say so rather than picking — the caller has context this repository does not.
- Search reads the same compiled-in catalogue as everything else here: offline, no network, no
  filesystem read at query time.
