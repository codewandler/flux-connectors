---
id: C-534
title: "The catalog artifact replaces compiled Flux (epic)"
pillar: Bridge
status: in-progress
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-spec, connector-flux, connector-pack, connector-cli, catalog, docs]
note: "EPIC — Decision 0022: compile the IR to a canonical per-provider document and a compressed pack; resolve requests from data, not parsed Flux; retire connector-flux behind a byte-identical differential gate"
---

# The catalog artifact replaces compiled Flux

## Goal

Make the compiled form of a connector a versioned data artifact per
`../flux-roadmap/decisions/0022-connectors-compile-to-a-catalog-artifact.md`: one canonical
committed document per provider, one compressed pack read by every consumer, and a resolver that
derives request plans from the document instead of re-parsing emitted Flux — preserving the
secrets boundary, the fail-closed refusals, and review-equals-execution, while decoupling
catalogue data from the crates.io engine-line release train.

## Acceptance

- [ ] The repository contract, vision and public docs state the Decision 0022 boundary — request
      shaping is closed declarative data, behaviour stays in Flux at the Flux layer — and C-10/C-15
      are closed as superseded, honestly (C-535).
- [ ] Every provider emits a canonical, deterministic, committed `catalog/<name>.catalog.json`
      carrying the complete published surface including the request template and the surfaces that
      today reach no artifact, hashed in `connectors.lock` (C-536).
- [ ] The documents compile into one versioned, digest-carrying pack, served by a dependency-free
      reader that preserves the existing `catalog` API; `codewandler-connector-catalog` becomes a
      shim over the embedded pack without a breaking API change (C-537).
- [ ] `connector-pack` resolves and projects from document data; the Flux parse is off the resolve
      path; a differential gate proves byte-identical request plans across the whole catalogue
      before the old derivation is deleted (C-538).
- [ ] Exchange consumes the reader and resolver from a schema release and holds zero runtime Flux
      parses; `.connector.toml` remains emitted as a projection until flux/D-214 repoints (C-539).
- [ ] `connector-flux`, `connectors/*.flux`, `crates/catalog/ops/**` and the generated Rust are
      deleted in the same release train as their replacement's proven adoption (C-540).

## Progress

- 2026-08-12: Decision 0022 accepted; design `docs/designs/catalog-artifact.md` recorded; child
  stories C-535…C-540 filed. No implementation begun.

## Notes

- Evidence for the defect this epic closes is recorded in the design with the measuring commands;
  re-measure before quoting any of its numbers.
- Sibling work: Exchange adoption stories are filed in `../flux-exchange`; Flux owes nothing at the
  invoke seam (its embedded Exchange client is the unchanged consumer).
