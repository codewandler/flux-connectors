---
id: C-112
title: Publish Flux core specifications in the connector explorer
pillar: UX
status: done
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

- [x] A failing-first ingest test accepts the versioned Flux core bundle and refuses schema-version,
      URI, duplicate, kind, availability, and dangling-schema-reference defects.
- [x] The public catalogue gains an additive `core` document, without adding core entries to the
      Rust connector catalogue, generated Flux modules, or connector-operation counts.
- [x] Every core entry is published at its canonical
      `https://flux.codewandler.org/v1/core/...json` `$id`, and the catalogue/entry/AST JSON Schemas
      are published below `/v1/schema/`.
- [x] The explorer searches and filters operations, nodes, and capabilities; renders static detail
      pages; links the JSON specification; and keeps available/planned and all four count families
      distinct.
- [x] HTTP, map/filter and the rest of the curated foundational set appear as available; `return`
      appears as a language node; `noop` is not invented; DNS/TCP/UDP/ICMP are visibly planned and
      never callable.
- [x] The Pages build is custom-domain safe at `flux.codewandler.org`, with root-relative assets and
      a committed CNAME, and all generated/public output is deterministic.
- [x] Rust and Node gates pass, including a second generation/diff fixed-point check.

## Progress

- Vendored Flux C-283's byte-stable export, validated it against embedded JSON Schemas and structural
  invariants, and published 77 entries plus three schemas below `/v1/`.
- Added searchable core cards and static detail pages, canonical spec links, root-domain deployment,
  and explicit non-callable treatment for the four planned protocol capabilities.
- Full Rust and Node gates pass; a second generation reports all 236 artifacts current.

## Notes

- Existing connector OIP/GID addresses remain runtime identities. The HTTPS namespace is the
  dereferenceable identity for the new core-spec records.
