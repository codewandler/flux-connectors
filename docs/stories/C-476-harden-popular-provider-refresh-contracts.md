---
id: C-476
title: "Make popular-provider refresh contracts fail closed"
pillar: Spec
status: done
priority: 13
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [providers, openapi]
note: "integration prerequisite — enforce full operation-id uniqueness/inventory and scrub values without deleting declarations"
---

# Make popular-provider refresh contracts fail closed

## Goal

Make every refresh script enforce the source-policy claims the epic already makes, including complete
operation-id inventories and example-value scrubbing that cannot erase a declaration named
`example` or `examples`.

## Acceptance

- [x] GitHub, Stripe, Microsoft Graph, and OpenAI refreshes count every outbound `paths` HTTP
      operation and refuse a missing or duplicate `operationId`; Stripe also pins its
      OpenAPI/version and path/operation inventory. Callback and root `webhooks` Operation Objects
      are outside this outbound connector inventory.
- [x] GitHub and OpenAI remove example keyword values while preserving declaration-map entries whose
      literal schema/component/property name is `example` or `examples`.
- [x] Mutation fixtures prove a duplicate/missing id, inventory drift, and a declaration-shaped
      `example` survive or refuse in the intended direction.
- [x] Deterministic offline replay reproduces Stripe, Microsoft Graph and OpenAI byte for byte; the
      GitHub output changes only by restoring named Example Object declarations with their
      `value`/`externalValue` removed, and then becomes a fixed point.
- [x] Provider selection, provenance, and scoped diff tests remain green.

## Progress

- 2026-08-02: independent review re-fetched all four official sources and measured the current
  documents as GitHub 1,216/1,216, Stripe 621/621, OpenAI 288/288, and Microsoft Graph selected
  extracts 1/1 plus 3/3, with no missing or duplicate operation ids. The current bytes are sound;
  the finding is that three refresh scripts do not yet prove those totals and two recursively delete
  declaration keys as well as example keywords.
- 2026-08-02: re-running the GitHub scrub disproved the review's narrower claim that the pinned
  source had no declaration-shaped occurrence: `jq '.components.examples | length'` measured 535.
  The corrected scrub retains those 535 names and removes every Example Object `value` or
  `externalValue`; its vendored SHA-256 is
  `a7a3293268f062d09eb576806f0e5af271c039b9427f9852c8280c8fecc692e7`.
- 2026-08-02: replayed all four scripts from independently downloaded bytes whose SHA-256 values
  matched the recorded official sources. Stripe, Graph and OpenAI stayed byte-identical; a second
  GitHub run reproduced the corrected bytes and provenance exactly. The five focused selection and
  provenance binaries passed 19 tests, and GitHub's scoped diff reported
  `12 artifacts up to date (1 provider checked)`.
- 2026-08-02: follow-up review found two over-broad claims. The scrubber now protects JSON Schema
  declaration maps such as `$defs` and `patternProperties` while treating component extensions as
  values, and its mutations independently prove missing, duplicate and total-inventory drift. The
  inventory contract is explicitly the outbound `paths` surface: OpenAI's root `webhooks` contains
  16 callback Operation Objects without `operationId`, and those are not selectable outbound calls.
- 2026-08-02: the corrected shared mutation suite and all three GitHub provenance tests pass. Fresh
  official-byte replays kept GitHub at vendored SHA-256
  `a7a3293268f062d09eb576806f0e5af271c039b9427f9852c8280c8fecc692e7` and reproduced OpenAI's
  committed bytes with 182 outbound paths, 288 outbound operations, 964 scrubbed example values and
  244 retained referenced components.
