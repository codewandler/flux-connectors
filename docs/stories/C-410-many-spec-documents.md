---
id: C-410
title: "One connector, many spec documents — a spec per service"
pillar: Spec
status: in-progress
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
- [x] `[[spec]]` accepts several entries, each naming the service its operations join. A single
      `[spec]` block keeps working unchanged — the existing golden errors
      (`patch-without-spec`, `nothing-to-generate`) still produce their exact messages.
- [x] `Provider::spec()` (`crates/connector-cli/src/discovery.rs:39`) no longer silently picks the last
      file by version order. A failing-first test puts two documents in one provider's spec directory
      and asserts both reach the IR.
- [x] Documents may disagree about security: the manager document declares root `oauth2` with **zero**
      operation overrides; `task-automation` declares per-operation `bearerAuth`+`oauth2` on all 31.
      Both resolve against the connector's `default_auth` without one document's model overwriting the
      other's.
- [x] A patch names the service it applies to, so two documents declaring the same `operationId` do not
      collide silently.
- [x] Provenance is per document — one `sha256` per spec, not one per connector — so drift-check can
      say *which* document moved.

## Progress
- **Done, gate green, `diff` still reports `557 artifacts up to date`.** No shipped artifact moved:
  no provider declares `[spec]` yet, and `Provenance::specs` is elided from the encoded IR when
  empty, so no `ir_sha256` moved either.
- **The shape.** `[spec]` and `[[spec]]` are one key in two TOML spellings, read by a
  `deserialize_with` visitor (`provider.rs::one_or_many_specs`) that dispatches on map-vs-seq and
  delegates to `SpecSource`'s own `Deserialize`. Deliberately *not* `#[serde(untagged)]`: untagged
  buffers the input and reports `data did not match any variant`, discarding the
  `deny_unknown_fields` key list and `toml`'s span — and this loader's error text is a deliverable.
  `an_unknown_key_in_a_spec_block_is_still_named_in_both_spellings` pins that.
- **The mapping.** `SpecSource.service` names the service a document joins; `IngestedDocument`
  carries one ingest per document, keyed by that service; a selected operation lands in it. Before
  this, every selected operation landed in `DEFAULT_SERVICE`, so a provider declaring named services
  beside a `[spec]` was a loud loader error.
- **A document joins a service, it does not declare one.** `[[spec]] service` must name a
  `[[services]]` entry, and two documents may not share one — a service is one name namespace and
  two vendor documents can declare one `operationId`.
- **Patch resolution.** `OperationPatch.service` is required as soon as a second document is
  declared, and optional with exactly one (which is what keeps the single-`[spec]` form unchanged).
  The duplicate-`select` check widened from `select` to `(service, select)`, so `getUser` selected
  out of two documents is two operations rather than a collision.
- **Discovery.** `Provider::spec()` is deleted rather than taught to choose better: the choice is the
  provider file's, and no caller existed. `Provider::specs` (the whole cache) is what the seam passes
  down, unchanged.
- **Not in scope, deliberately.** `openapi::ingest` still extracts no `security` — that is C-5 — so
  the security acceptance is proved at the level the IR can express today: the ingests are kept apart
  one per service, and an `auth` override stated on one service's patch does not reach the other's
  operation, which still inherits `default_auth`. `LockEntry` keeps its single-document shape; with
  several documents `Provenance::spec_sha256` is `None` and `Provenance::specs` is the record, so
  widening the lockfile is C-7/C-14's to do.

## Notes
- The service model this rides on is C-66's (provider → service → members, one name namespace per
  service); this story adds no new grouping concept.
- The two API versions (`/api/v2`, `/api/v3`) live in the operation paths, exactly as the nine current
  babelforce operations already carry `/api/v2/`. Do not introduce a per-service base URL for this.
- Sequenced after C-4: ingest one document correctly before ingesting five.
