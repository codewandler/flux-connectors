---
id: C-524
title: "RFC Editor corpus is a searchable, locally cached datasource"
pillar: Connector
status: backlog
design: docs/designs/vendor-datasource-declarations.md
epic: vendor-datasources
areas: [providers, datasources, runtime, cache, search]
note: "index every RFC from the authoritative RFC Editor corpus once, then serve bounded full-text search and get from an atomic local snapshot with offline last-good reuse"
---

# RFC Editor corpus is a searchable, locally cached datasource

## Goal

Expose the complete public RFC Series as one read-only connector datasource: an agent can search
metadata and full text across every RFC, retrieve an exact RFC or section with a canonical citation,
and reuse a durable local corpus snapshot without paying network latency on each read.

This is an official external integration, so flux-connectors owns the RFC Editor declaration and
runtime adapter. A locally placed Exchange may keep the cache on the same machine, but Flux does not
gain a vendor-specific fetcher or a second local fallback.

## Acceptance

- [ ] A design review fixes the source, refresh, cache and indexing contract before implementation.
      It uses the RFC Editor's authoritative corpus and supported mirror/index surfaces—not its web
      search UI—and pins the allowed origins. The reviewed inputs include the text corpus exposed by
      `rsync.rfc-editor.org::rfcs-text-only`, `rfc-index.xml`, and canonical
      `https://www.rfc-editor.org/rfc/rfcNNNN.txt` identities; an unavailable upstream never causes a
      silent fallback to a third-party mirror.
- [ ] The connector declares read-only RFC search/list and get operations plus a `[[datasources]]`
      member after C-512/C-513. The datasource entity contract has stable RFC and section identities,
      canonical URLs, title, authors, publication date, stream/category/status, abstract, obsoletes/
      updates relations and normalized searchable body text. RFC numbers remain strings at the wire
      boundary and the same identity is returned by search, get and relation traversal.
- [ ] Search covers the full locally mirrored corpus, not only titles or recently published RFCs.
      Number, title, author, abstract and body terms are searchable; exact RFC-number matches rank
      first; filters cover at least status/category and publication year; deterministic ranking and
      opaque paging return bounded hits, snippets and total bytes.
- [ ] A host-owned prepare/refresh lifecycle builds a versioned on-disk snapshot and full-text index
      before advertising readiness. Cold bootstrap is explicit and observable rather than hidden in
      the first model read. Refresh is incremental after bootstrap, single-flight across concurrent
      workers, stages and validates all changed bytes, then atomically swaps the manifest/index so
      readers see either the previous complete snapshot or the next complete snapshot—never a
      partially refreshed corpus.
- [ ] Every search/get call reads the local snapshot only and performs zero network requests. A
      complete last-good snapshot remains queryable while refresh or the RFC Editor is unavailable;
      results report snapshot identity, source provenance, synchronized-at time and freshness. With
      no valid snapshot, the datasource is not advertised as ready and returns a typed operator-facing
      remediation instead of an empty corpus.
- [ ] Cache ownership, placement and bounds are explicit. Files live under the runtime host's
      configured state/cache root, never the workspace or ambient home directory; the manifest
      versions the schema/index format and records per-input digests; RFC bodies are stored once;
      corrupt, incompatible or truncated state is quarantined/rebuilt without discarding the last
      verified snapshot. Reads remain safe during rebuild, process restart and concurrent access.
- [ ] The runtime artifact is connector-owned and executes only through Exchange under the existing
      admitted-operation and artifact-verification contracts (C-497/C-498). It requires no vendor
      credential, cannot accept a caller-selected URL/path/mirror, declares exact network and local
      storage effects for refresh, and passes no fetched markup or bytes to a shell or executable
      interpreter.
- [ ] Failing-first hermetic fixtures cover: complete corpus enumeration, metadata and body-term
      search, exact-number ranking, section retrieval and citations, filters/paging/bounds, a warm
      cache with transport forced offline, incremental refresh, interrupted refresh, corrupt state,
      format migration, concurrent readers/refreshers and refusal of unexpected origins or oversized
      inputs. Tests do not contact the RFC Editor or require `rsync` on the test host.
- [ ] Operator and agent documentation explains initial sync cost, cache location/size and freshness,
      explicit refresh/repair, offline behavior, corpus identity in results, and how to cite the RFC
      Editor rather than presenting cached text as current implementation guidance. The complete
      connector fixed-point, Rust gate and applicable Exchange conformance journey pass.

## Progress

- 2026-08-06: Filed from the request to expose `https://www.rfc-editor.org/rfc/*` as a datasource
  that searches all RFCs and caches them locally. Upstream reconnaissance confirmed that the RFC
  Editor publishes canonical per-RFC text, an RFC index, and an official `rfcs-text-only` rsync
  module specifically for keeping a local mirror current. No implementation has started.

## Notes

- Architecture authority:
  `../flux-roadmap/decisions/0006-datasources-are-declared-read-surfaces.md`, especially rules 5–8
  and 12; connector shape:
  [vendor-datasource-declarations.md](../designs/vendor-datasource-declarations.md); runtime placement:
  [all-integrations-are-connectors.md](../designs/all-integrations-are-connectors.md).
- Scheduling prerequisites: C-497, C-498, C-512 and C-513, plus the Exchange tenant Datasource bind
  and read seam. Add those edges to the cross-repository Board before moving this story to `ready`.
- The cache is an adapter-owned acceleration and availability snapshot inside the Exchange runtime
  placement. It is not Flux's indexed `DatasourceBackend`, not a new datasource access mode, and not
  a local vendor adapter fallback.
- RFC errata are explicitly out of the first corpus unless the design can add them as separately
  identified RFC Editor records without changing the immutable RFC body or overstating their status.
