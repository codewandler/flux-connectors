---
id: C-110
title: Ship the Linear connector — or record why a GraphQL vendor cannot be one
pillar: Spec
status: in-progress
priority: 6
design:
epic: provider-fleet-2
areas: [providers, connector-spec]
note: "GraphQL-only. The pipeline is REST-shaped; this either proves it stretches or produces a documented refusal. Either outcome is worth more than another REST connector"
---

# Ship the Linear connector — or record why a GraphQL vendor cannot be one

## Goal
Answer a question the catalogue has never asked: can a connector describe a vendor with **one
endpoint and a query language**?

## Acceptance
- [x] Either a working Linear connector, **or** a written finding that the operation model cannot
      describe a GraphQL vendor honestly, with the specific reason. Both are acceptable outcomes and
      the story is not a failure if it ends in the second.
      → **It ships.** `providers/linear.toml`, 8 operations. The finding is written *as well as* the
      connector: the header of that file and of `crates/connector-flux/tests/linear_connector.rs`
      record the two places the model does not stretch, and both are asserted rather than described.
- [x] If it ships: operations are `POST /graphql` with a **constant** query in the body and the
      variables as parameters — so each operation still has a real typed signature and a real
      response schema, rather than one `query: String` parameter a model has to author.
      → `crates/catalog/ops/linear/linear-issue-get.flux` emits `op linear-issue-get(id: String)`
      with `query = """…"""` bound beside it and `payload = { query, variables: { id } }`.
      Asserted by `the_query_document_is_pinned_and_no_caller_can_choose_it`,
      `the_variables_are_typed_parameters_under_the_variables_key` and
      `every_response_schema_is_nested_under_data`.
- [x] **Refuse the tempting wrong shape.** A single `linear-graphql(query)` operation that takes an
      arbitrary query string is exactly the remote-expression-evaluator the vision rejects: it hands
      a model a language instead of an operation, and nothing about it is typed, curated or
      analyzable.
      → No operation exposes `query`. `the_query_document_is_pinned_and_no_caller_can_choose_it`
      asserts the emitter's parameter list omits it on every operation, so the shape is refused by a
      test rather than by intent.
- [x] If it ships: auth is a bearer API key; a `[[config]]` surface; a `verify` operation; a
      per-provider contract test.
      → `linear.api_key` (`scheme = "bearer"`), one `[[config]]` field binding it,
      `verify = "linear-viewer"`, and `crates/connector-flux/tests/linear_connector.rs` (11 tests).
      Asserted by `the_connector_authenticates_with_one_bearer_key_and_verifies_with_a_read`.
- [ ] If it does not ship: the finding lands in `docs/designs/` and this story closes `done` with the
      finding as its deliverable, not `blocked`.
      → **Not applicable — it shipped.** No `docs/designs/` record was written: the findings are
      per-connector rather than cross-cutting, so they live in the two files that carry them and in
      the two follow-up stories named under Progress.

## Progress
- **Shipped, 2026-07-31.** `providers/linear.toml` — 8 operations, all `POST /graphql`, each with a
  build-time-constant query document and typed GraphQL variables as its parameters.

### What the probe measured

Four things worked, three of them with no new mechanism at all:

1. **Nothing keys an operation by its path.** Identity is `id` only; `catalog::Operation` has no
   `path` field. Eight operations on one endpoint is a difference of degree from `zendesk`'s three on
   one `PUT` path, not of kind. This was the finding most likely to sink the story and it was a
   non-event.
2. **C-55's constant body field genuinely covers a query document**, rather than merely resembling
   the case — which is what this story's note asked to be checked. `op.rs`'s `constant` is a bare
   `schema.get("const")`: no type, length or newline restriction, and the emitter filters constants
   out of the operation's signature. Sent on every call, declarable by nobody.
3. **A multi-line document survives the emitter** as a verbatim `"""…"""` block. No provider had
   exercised that path before; `a_multiline_query_document_round_trips_through_the_emitter` is the
   first test in the repository that does.
4. **The `data.<field>` envelope is declarable**, and the response schema is *stronger* than a REST
   one exactly as this story's Notes hoped: the shape under `data` is a consequence of the pinned
   document, and the test asserts the two agree.

### Two things did not work, and are recorded rather than papered over

5. **Every operation is forced to `risk >= medium` and `idempotency = non_idempotent`, reads
   included.** `check_write_metadata` derives write-ness from the HTTP verb, and under GraphQL the
   verb is `POST` for everything. The risk floor rises from `low` to `medium` for the whole
   connector, and `idempotency` carries no authored information anywhere in the file — no operation
   could have said anything else. It ships anyway because the forced value is *conservative* (a read
   over-stated, never a write under-stated) and gradation above the floor survives
   (`linear-issue-archive` is `destructive`). The axis is **compressed, not erased**.
   `a_graphql_read_cannot_declare_itself_low_risk` and `a_graphql_read_cannot_declare_itself_idempotent`
   pin it by constructing the declaration a REST author would have written and asserting the refusal.
6. **A failed Linear call arrives as HTTP 200 and nothing here can say so** — C-57's exact case, and
   the safety-relevant result of this story. Linear signals every failure with `200`, a `null` `data`
   and an `errors` array; this repository's success signal is the transport's. So the connector
   declares **no** `error_envelope` anywhere, because declaring one makes `description()` append *"A
   non-2xx response is returned as data…"* — false for Linear, and it points a model at a branch that
   never occurs. The only mitigation available is prose, so every operation's `description` ends with
   the same sentence about checking `errors` first.
   `no_operation_declares_an_error_envelope_because_the_prose_it_emits_would_be_false` pins the
   decision *with* its reason, so closing C-57 breaks the test and forces a revisit.

A third, smaller finding: `linear-issue-list` pages properly but declares no `quirks.pagination`,
because `Pagination::Cursor.cursor_param` names a *query parameter* and a GraphQL cursor is a body
variable. That is C-57's fourth acceptance item; Linear is the second provider to hit it after Slack.

### Follow-up worth filing
- **A `POST` that is semantically a read cannot say so.** Not a request to weaken
  `check_write_metadata` — it should stay exactly this strict for REST — but the IR has no way for an
  operation to declare that its verb is a transport detail. GraphQL makes this total rather than
  occasional.
- **C-57 now has a second, sharper consumer.** Slack's `{"ok": false}` is the filed case; Linear is
  the case where *no* call is status-signalled, and the follow-on effects (the false envelope prose,
  the unexpressible body cursor) are both visible in one connector.

## Notes
- The constant-query shape leans on the same mechanism as a constant body field, which the emitter
  already reads from a JSON Schema `const` — read `crates/connector-flux/src/op.rs`'s `constant`
  before designing anything new.
- Response schemas are the weak point: a GraphQL response is shaped by its query, which is the one
  place this model should be *stronger* than REST, since the query is fixed at build time.
