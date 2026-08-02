---
id: C-464
title: "Add Zendesk Messaging from the Sunshine Conversations spec"
pillar: Spec
status: done
priority: 2
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [providers, connector-flux]
note: "nine curated conversation, message, user and participant operations; webhook responses expose a signing secret and remain withheld"
---

# Add Zendesk Messaging from the Sunshine Conversations spec

## Goal
Publish the approved Messaging surface as a named, spec-backed Zendesk service.

## Acceptance
- [x] Messaging has a distinct service contract for host, version, authentication, configuration,
      and tags; `GetConversation` is its bounded diagnostic read while the single connector-level
      verifier remains Support's already-published `zendesk-test`.
- [x] Nine conversations, messages, users and participant selections exactly match C-460's corrected
      inventory and remain opt-in; the five webhook lifecycle operations remain absent.
- [x] Multipart and credential-returning operations are withheld; any write's risk/idempotency is
      stated explicitly.
- [x] Existing Support and Help Center addresses/artifacts do not move.
- [x] Scoped build/diff and request rehearsal prove every emitted Messaging operation composes.

## Progress

- 2026-08-02: read-only implementation preflight confirmed all fourteen originally proposed
  operationIds, methods and paths, but found the Create/Get/Update webhook response schemas expose
  `webhook.secret`, while List exposes the same signing credential for every item. C-430 makes those
  four withholds; Delete is deferred with the family rather than published as an orphaned destructive
  operation. The implementation boundary is therefore the nine conversation/message/user operations.
- 2026-08-02: the connector IR has one connector-level verifier, not one per service, and every
  approved Messaging read requires a caller-owned resource id. The story therefore preserves
  Support's `zendesk-test` verifier and proves `GetConversation` as Messaging's low-risk diagnostic
  read without claiming an unattended per-service verification facility the IR cannot express.
- 2026-08-02: spec operation patches deliberately cannot state `repeatable_because`, so C-186
  refuses `conditional` on a selected operation. Both updates are narrowed to one required absolute
  target-state field and request rehearsal proves byte-identical replay. They remain conservatively
  `non_idempotent`, because Flux also uses `idempotent` to license cached-result substitution and
  refuses that stronger claim for every PATCH.
- 2026-08-02: failing-first ingest evidence found both message endpoints' response graph is
  recursive: `message -> quotedMessage -> quotedMessageMessage -> message`. The finite OpenAPI IR
  therefore skips `PostMessage` and `ListMessages`. They use the repository's supported mixed
  front-end with methods, paths, a narrowed text-message body, and bounded non-recursive response
  members transcribed from the pinned document; a negative regression keeps the two spec patches
  absent until recursive-schema support makes exact selection possible.
- 2026-08-02: focused gates are green: `zendesk_messaging` is 6/6, Messaging rehearsal is 3/3,
  workspace build succeeds, and focused clippy succeeds with warnings denied. Scoped fixed-point
  checks report `2 artifacts up to date (1 provider checked)` for the Messaging service and
  `36 artifacts up to date (1 provider checked)` for Zendesk. Support and Help Center SHA-256 fences
  remain byte-identical. The first workspace run found six stale Zendesk closed-set assertions plus
  five expected whole-catalogue stale targets. A targeted rerun makes all six Zendesk assertions
  green; its only remaining red is the concurrent GitHub story's still-stale inventory count. The
  coordinator owns the five generated whole-catalogue files at integration.

## Notes
- This is Zendesk Messaging/Sunshine Conversations, not the deprecated legacy Chat API.
