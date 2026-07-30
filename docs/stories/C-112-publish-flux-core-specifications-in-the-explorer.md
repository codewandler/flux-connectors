---
id: C-112
title: Publish Flux core specifications in the connector explorer
pillar: UX
status: in-progress
priority: 1
design: docs/designs/core-catalogue.md
epic: explorer-ux
areas: [connector-cli, website, catalog]
note: "Built-ins and language nodes become searchable beside connectors, with canonical JSON identities rather than fake providers"
---

# Publish Flux core specifications in the connector explorer

## Goal

Make Flux's foundational operations, language nodes, and network-capability roadmap discoverable in
the public explorer while preserving the ownership boundary: Flux defines built-ins; this repository
vendors and publishes their inert specifications.

## Acceptance

- [ ] A failing-first ingest test accepts the versioned Flux core bundle and refuses schema-version,
      URI, duplicate, kind, availability, and dangling-schema-reference defects.
- [ ] The public catalogue gains an additive `core` document, without adding core entries to the
      Rust connector catalogue, generated Flux modules, or connector-operation counts.
- [ ] Every core entry is published at its canonical
      `https://flux.codewandler.org/v1/core/...json` `$id`, and the catalogue/entry/AST JSON Schemas
      are published below `/v1/schema/`.
- [ ] The explorer searches and filters operations, nodes, and capabilities; renders static detail
      pages; links the JSON specification; and keeps available/planned and all four count families
      distinct.
- [ ] HTTP, map/filter and the rest of the curated foundational set appear as available; `return`
      appears as a language node; `noop` is not invented; DNS/TCP/UDP/ICMP are visibly planned and
      never callable.
- [ ] The Pages build is custom-domain safe at `flux.codewandler.org`, with root-relative assets and
      a committed CNAME, and all generated/public output is deterministic.
- [ ] Rust and Node gates pass, including a second generation/diff fixed-point check.

## Progress

- Design accepted; waiting on the Flux-owned export contract in flux C-283.

## Notes

- Existing connector OIP/GID addresses remain runtime identities. The HTTPS namespace is the
  dereferenceable identity for the new core-spec records.

