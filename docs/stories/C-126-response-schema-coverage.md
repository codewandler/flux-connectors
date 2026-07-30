---
id: C-126
title: "Raise response_schema coverage and put a floor under it"
pillar: Spec
status: done
design: docs/designs/member-io-schemas.md
epic: member-io
areas: [providers, connector-spec]
note: "Re-measured on entry: 29 of 110, not the design's 16 of 97. Now 92 of 110 with a floor test under it — coverage that nothing watches only ever goes down"
---

# Raise response_schema coverage and put a floor under it

## Goal

Declare response shapes for the operations that lack them, and make the coverage figure a measured,
non-decreasing property rather than an accident.

## Acceptance

- [x] **The floor test lands first, before any new schema.** A test reports current
      `response_schema` coverage and fails if it drops below the recorded floor. Measured today:
      ~~**16 / 97**~~ → **29 / 110**, re-measured through the loader: the design's figure predates
      stripe and notion.
- [x] Coverage rises meaningfully across the shipped providers, prioritising the operations a caller
      most needs to destructure — reads that return a single entity, and anything a flow branches on.
- [x] Every schema added is **derived from the vendor's published documentation**, cited in the TOML
      the way `docs/designs/provider-operation-inventory.md` already cites wire shapes. A guessed
      schema is worse than none: it looks authoritative and is not.
- [x] **Absence stays absence.** An operation whose response shape is genuinely unknown emits no
      schema — never `{}` and never a permissive `{"type": "object"}`, both of which pass a coverage
      count while telling a consumer nothing.
- [x] The floor is raised to the new figure in the same commit, so the ratchet only turns one way.
- [x] Generation stays **offline**: no operation's schema may be obtained by calling the vendor. That
      rule is absolute (`AGENTS.md`).
- [x] The gate is green; the build stays a fixed point.

## Notes

- **The floor is the deliverable; the number is the by-product.** Coverage nothing watches only ever
  decreases — a new connector ships without response shapes and the ratio quietly falls. The test is
  what makes the next hundred operations better.
- Do not chase 100%. Some vendor responses are genuinely unspecified or vary by account; recording
  that honestly is a better outcome than a schema nobody can rely on. Say in Progress which ones you
  deliberately left absent and why.
- This story declares what the **vendor sends**. It does not make that the operation's output type —
  that distinction is [C-127](C-127-truthful-output-typing.md), and conflating them is the failure
  mode the epic is built around.
- Beware error envelopes: several providers answer `200` with an error in the body (Slack's `ok`,
  Zendesk's flat-body silent ignore). A response schema that models only the success case is a
  half-truth; note the error shape where the inventory already documents it.

## Progress

**Measured, not assumed: 29 / 110 on entry, 92 / 110 (83%) on exit.** The design's 16 / 97 predates
stripe (8 / 8) and notion (5 / 5); the catalogue is 19 providers and 110 operations, not 105. The
figure is now produced by `crates/connector-spec/tests/response_schema_coverage.rs`, which reads
`providers/*.toml` through the real loader rather than `web/public/catalog.json` — the catalogue is a
whole-catalogue artifact a scoped build deliberately leaves stale, so measuring there would report the
last full build's number and call it today's.

### The floor test, which is the deliverable

Three tests, and the second and third are what make the first non-gameable:

1. `response_schema_coverage_does_not_fall_below_its_floor` — two floors, not one. `COVERED_FLOOR = 92`
   catches a deleted schema; `RATIO_FLOOR_PERCENT = 82` catches **operations arriving without
   response shapes**, which is the regression a count alone cannot see and the one that actually
   happened between the design's measurement and this story. It prints the per-provider table on every
   run.
2. `the_recorded_floor_is_the_measured_figure` — the ratchet's other direction. Coverage may run ahead
   of the floor by a tenth of the catalogue; beyond that the floor must be raised in the commit that
   earned it, so the recorded figure cannot quietly become archaeology.
3. `no_operation_publishes_a_permissive_response_schema` — refuses `{}`, `true`, and any schema with no
   `properties`/`items`/`required`/`$ref`/`oneOf`/`anyOf`/`allOf`/`const`. Absence stays absence
   because a placeholder is *enforced* to be unpublishable, not because an author remembered.

### Where the 63 new schemas came from

Every one is read from the vendor's own published reference, cited in the TOML beside it. Machine-
readable sources where they exist — Jira's `swagger.v3.json`, Sentry's per-page OpenAPI fragments,
HubSpot's rendered OpenAPI schemas, Fly's `openapi3.json`, Intercom's generated docs — and the
vendor's reference pages otherwise (Zendesk, Freshdesk, GitHub, Google, Shopify, Zoom, Slack, OpenAI,
OpenRouter). **Nothing called a vendor API**; the build fetches nothing, and `no_network.rs` is
untouched.

The schemas are weighted towards what a consumer must destructure, and the recurring finding is that
the *envelope* and the *type* are what cannot be inferred:

- **Type traps, published where a flow will read them.** HubSpot returns every property value as a
  string (`"amount": "1500.00"`) — schema-guaranteed via `additionalProperties: {type: string}`.
  Sentry's `count` is a string while `userCount` beside it is an integer. Freshdesk's `status`/
  `priority` are integers with documented mappings (2 Open · 4 Resolved), where zendesk's are strings
  (`solved`). Shopify's `tags` is a comma-separated **string**, and its money is decimal strings. Zoom's
  `type` is an integer plan code and its `verified` is an integer, not a boolean. Gmail's
  `internalDate` is epoch-milliseconds in a string.
- **Three-valued and null-as-a-value fields.** GitHub's `mergeable` is `true`/`false`/**null meaning
  not yet computed** — a gate reading it as a boolean sends the unknown case down the merge path.
  Shopify's `fulfillment_status` null *is* "unfulfilled". Intercom's `open` is true for a snoozed
  conversation, so `state` and `open` are both needed.
- **Error envelopes that arrive with 200.** Slack's four operations model `ok`/`error` in one schema
  with `ok` as the only required member — a success-only schema would promise a `ts` that is not there
  on `channel_not_found`. OpenRouter's per-choice `error` is the same class: HTTP 200, well-formed
  completion object, no content. Zendesk's writes answer `{ticket, audit}`, and `audit.events` is the
  only place a flat-body write that was accepted-and-ignored differs from one that applied.
- **Envelopes proper.** Jira's issue data is entirely under `fields`; its create returns *only*
  `id`/`key`/`self`. Asana-style `data` wrappers on OpenRouter differ between its two list operations
  (`data` as array vs. `data` as object). OpenAI's embeddings answer with a list even for a single
  input.

### The eighteen left absent, and why each one is

`babelforce 9` — no public API reference exists, and the one authoritative document
(`manager.openapi.json`) cannot be vendored: inventory §1.3 records that its response `example` blocks
are where credential-shaped values for a real test account live. The document that would supply these
schemas is exactly the document that cannot be quoted from until it is cleared and rotated. Writing
them from memory would be the guess this story forbids; `{}` × 9 would have moved the figure to
101 / 110 while saying nothing. `providers/babelforce.toml` carries the full reasoning.

`fly 4` — the vendor's own OpenAPI declares `machines_start`/`stop`/`restart`/`delete` with a `200` and
an **empty content object**. Nothing to derive.

`google 2` — Drive v3 returns a `fields` projection, this connector cannot send `fields` (C-30), and
Google does not document the default set for `files.get` or `files.update`. Tempting to write
`{kind, id, name, mimeType}`; refused, because that set is documented for `files.list` and two Google
pages disagree with each other even about *that* (four fields vs. five with `resourceKey`).

`jira 1`, `zoom 1` — `204 No Content`. `{"type": "object"}` would be both uninformative and false.

`hubspot 1` — the `PATCH …/contacts/{id}` reference renders no Responses section at all: no status, no
schema, no example. It almost certainly returns `SimplePublicObject`; "almost certainly" is the reason
nothing is declared.

### One reversal worth flagging to review

`providers/sentry.toml` carried a C-77 comment stating that **no** `response_schema` was declared
because "Sentry returns the resource itself with no envelope, so there is no shape a consumer could not
infer". That comment is replaced, with the reasoning inverted in place: an absent envelope makes the
top level inferable, not the members, and the string/integer split between `count` and `userCount` is
exactly the thing no consumer infers. Sentry is now 4 / 4.

### Not this story

Nothing here changes what a caller of an emitted op receives. `http.request` returns one flat string,
so `.data.id` against generated Flux still resolves to nothing; several schemas say so in their own
comments, and C-127 owns publishing the distinction. Adjacent and recorded, not fixed: `ErrorEnvelope`
has no success predicate, so Slack's `ok: false` and Stripe's `402` decline remain
indistinguishable from a transport failure to anything reading the envelope alone.
