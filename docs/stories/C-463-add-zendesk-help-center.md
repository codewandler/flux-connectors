---
id: C-463
title: "Add Zendesk Help Center as a named service"
pillar: Spec
status: done
priority: 2
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [providers, connector-flux]
note: "articles, categories, sections, and translations from the pinned Help Center OAS after the legacy-default service prerequisite"
---

# Add Zendesk Help Center as a named service

## Goal
Publish the approved knowledge-base surface as a real named service without changing Support.

## Acceptance
- [x] The service has its own name, description, base URL/version resolution, auth/config binding,
      tags, and verification read.
- [x] The seven expressible article, category, section, and translation selections exactly match
      C-460's corrected inventory; selection remains
      opt-in and new upstream paths stay absent until reviewed.
- [x] Writes carry explicit safety/idempotency and multipart or response-secret operations stay out.
- [x] Existing Support OIPs, credential path, and unsuffixed artifacts are byte-stable; Help Center
      emits only named-service artifacts and addresses.
- [x] Scoped build/diff, request rehearsal, and workspace tests cover both services.

## Progress

- 2026-08-02: stopped before implementation on a pinned-contract contradiction. Running
  `yq -o=json '.' specs/zendesk/help-center-2026-08-02.openapi.yaml | jq -c
  '.paths["/api/v2/help_center/articles/{article_id}"].put | {operationId, requestBody,
  responses}'` reports `{"operationId":"UpdateArticleNoLocale","requestBody":null,...}`. The
  vendored YAML at lines 2809–2823 likewise declares only the operation id, tags, summary, and 200
  response. C-460 freezes this row as a JSON target-state update, but the current overlay cannot add
  a request body. Selecting it would therefore emit a bodyless `PUT`; hand-authoring around the
  pinned document would violate this story's spec-backed boundary.
- 2026-08-02: after C-460 corrected that row to `Defer`, failing-first
  `cargo test -p codewandler-connector-spec --test zendesk_help_center -- --nocapture` reported 3
  failed / 1 passed: only `default` existed, zero of seven Help Center patches existed, and the first
  Help Center read was absent. The matching pack rehearsal failed because
  `zendesk-help-center-category-list.flux` did not exist.
- 2026-08-02: the provider now declares legacy `default` beside named `help-center`, pins the Help
  Center document to that service, and selects exactly seven operationIds. Six reads are
  low/idempotent; article creation is high/non-idempotent and retains only the required top-level
  `article` body. The bodyless update and translation writes, search, and attachments remain absent.
- 2026-08-02: `cargo test -p codewandler-connector-spec --test zendesk_help_center --test
  zendesk_spec_selection --test vendored_zendesk_specs -- --nocapture` passed 13 tests. The hash
  fence re-measured both unsuffixed artifacts and all 13 existing Support operation renderings as
  byte-identical, while addresses stayed under `com.zendesk.api:v2#…` and the credential path stayed
  `tenants/tenant-1/com.zendesk.api/api_token`.
- 2026-08-02: both Zendesk pack rehearsals passed. The seven Help Center cases composed absolute,
  brace-free requests against `https://acme.zendesk.com`; article creation composed the documented
  nested JSON body and the other six emitted no body.
- 2026-08-02: the scoped build reported `1 provider, 25 artifacts; 10 written`; only the two named
  Help Center unit artifacts, seven operation renderings, and the provider catalogue table were
  written. `diff --provider zendesk --service help-center` then reported `2 artifacts up to date (1
  provider checked)`, and full provider diff reported `25 artifacts up to date (1 provider
  checked)`. Two focused `service_units` tests also passed, proving shipped multi-service narrowing
  and emitted-unit counts.
- 2026-08-02: the catalogue-wide request-composition test currently reaches an unrelated concurrent
  Twilio failure first: `twilio-recording-list` has no declared `username.twilio.basic_auth` config
  field. The dedicated Support and Help Center rehearsals are green; no Zendesk failure was observed.

## Notes
- C-458 and C-459 are complete; C-463 consumes their mixed-service and pinned-document contracts.
