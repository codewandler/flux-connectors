---
id: C-410
title: "One connector, many spec documents — a spec per service"
pillar: Spec
status: backlog
priority: 2
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec, connector-cli]
note: "discovery.rs:39 returns the LAST spec by version order and SpecSource.path is one string — one document per provider was never decided, it was assumed. babelforce has five, over two API versions and two security models"
---

# One connector, many spec documents — a spec per service

## Goal
Let a connector ingest several vendor documents, each becoming one service, so a vendor that splits its
API across documents does not have to become several connectors.

## Acceptance
- [ ] `[[spec]]` accepts several entries, each naming the service its operations join. A single
      `[spec]` block keeps working unchanged — the existing golden errors
      (`patch-without-spec`, `nothing-to-generate`) still produce their exact messages.
- [ ] `Provider::spec()` (`crates/connector-cli/src/discovery.rs:39`) no longer silently picks the last
      file by version order. A failing-first test puts two documents in one provider's spec directory
      and asserts both reach the IR.
- [ ] Documents may disagree about security: the manager document declares root `oauth2` with **zero**
      operation overrides; `task-automation` declares per-operation `bearerAuth`+`oauth2` on all 31.
      Both resolve against the connector's `default_auth` without one document's model overwriting the
      other's.
- [ ] A patch names the service it applies to, so two documents declaring the same `operationId` do not
      collide silently.
- [ ] Provenance is per document — one `sha256` per spec, not one per connector — so drift-check can
      say *which* document moved.

## Progress
- (not started)

## Notes
- The service model this rides on is C-66's (provider → service → members, one name namespace per
  service); this story adds no new grouping concept.
- The two API versions (`/api/v2`, `/api/v3`) live in the operation paths, exactly as the nine current
  babelforce operations already carry `/api/v2/`. Do not introduce a per-service base URL for this.
- Sequenced after C-4: ingest one document correctly before ingesting five.
