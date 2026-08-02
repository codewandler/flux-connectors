---
id: C-468
title: "Freeze the five-provider OpenAPI source and operation inventory"
pillar: Agent
status: done
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [providers, openapi]
note: "exact commits, hashes, selectors, parameters, response models and defer reasons before parallel implementation"
---

# Freeze the five-provider OpenAPI source and operation inventory

## Goal

Give five parallel implementors a command-backed, non-overlapping contract instead of asking each to
rediscover source identity and safe operation scope.

## Acceptance

- [x] For GitHub, Stripe, Microsoft Graph, OpenAI and Twilio, record the exact first-party URL,
      immutable commit/tag, OpenAPI version, byte hash, path/operation count and license posture.
- [x] Select at least four exact operation ids per provider and record method, path, required
      parameters, chosen query parameters, request/response schema and risk/idempotency.
- [x] Every omitted candidate has a concrete reason such as form encoding, multipart, caller-chosen
      host/path, credential-bearing output or missing schema; no generic "too risky" bucket.
- [x] Pin the existing operation identities and emitted Flux hashes that each provider story must
      preserve.
- [x] Predict each implementation write set and prove the five are disjoint outside coordinator-owned
      catalogue/changelog/board files.

## Progress

- 2026-08-02: `git ls-remote <first-party-repository> HEAD` re-measured the five immutable source
  commits recorded in the inventory; `curl -fsSIL` returned HTTP 200 for all five commit-pinned raw
  URLs, and re-fetching all five reproduced every recorded byte count and SHA-256.
- 2026-08-02: the section-table count is four exact operation rows for each provider. The source
  ledger has five rows, and all 30 current per-provider Flux files have an id/hash row matching
  same-session `sha256sum` output.
- 2026-08-02: OpenAI replaced HubSpot after the latter's first-party bytes proved to have no
  redistribution license. A scaffold over OpenAI's pinned 3,244,309-byte OpenAPI 3.1 document
  reported zero unread operations and one narrower diagnostic, confined to the unselected
  `GET /files/{file_id}/content` string response.
- 2026-08-02: implementation re-opened the pinned GitHub bytes and corrected two hand-counted
  parameter totals in the inventory: `actions/list-workflow-runs-for-repo` declares 12 parameters,
  not 13, and `repos/list-commits` declares 10, not 12. The source hash, selectors and curated
  integer `page`/`per_page` surface were unchanged.
- 2026-08-02: Stripe implementation disproved the first selector set: all four original collection
  responses reach `file -> file_link -> file`, not only the already-recorded product cycle. The
  amended exact set is `GetCountrySpecs`, `GetEvents`, `GetExchangeRates` and `GetBillingMeters`;
  current ingest retains each official 200 list envelope, and only integer `limit` is curated.
