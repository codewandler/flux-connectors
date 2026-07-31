---
id: C-110
title: Ship the Linear connector — or record why a GraphQL vendor cannot be one
pillar: Spec
status: done
priority: 6
design: docs/designs/graphql-vendors.md
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
      → **The finding.** No `providers/linear.toml`. `docs/designs/graphql-vendors.md` records the
      specific reason and the two test files pin it.
- [ ] If it ships: operations are `POST /graphql` with a **constant** query in the body and the
      variables as parameters — so each operation still has a real typed signature and a real
      response schema, rather than one `query: String` parameter a model has to author.
      → **Not applicable — it does not ship.** Established as *expressible* though, and pinned in
      `crates/connector-flux/tests/linear_connector.rs` so a later attempt does not re-derive it:
      `the_query_document_is_pinned_and_absent_from_the_signature`,
      `a_multiline_query_document_round_trips_through_the_emitter`,
      `a_response_schema_can_describe_the_data_envelope`.
- [x] **Refuse the tempting wrong shape.** A single `linear-graphql(query)` operation that takes an
      arbitrary query string is exactly the remote-expression-evaluator the vision rejects: it hands
      a model a language instead of an operation, and nothing about it is typed, curated or
      analyzable.
      → Refused, and never proposed. The finding is the opposite failure and is worth stating
      plainly: the pinned-constant shape *is* the right one at the compiler, and it is
      `connector-pack` that turns the constant back into something an operator can edit.
- [ ] If it ships: auth is a bearer API key; a `[[config]]` surface; a `verify` operation; a
      per-provider contract test.
      → **Not applicable — it does not ship.** All four were written and worked; the fixture in
      `linear_connector.rs` keeps the auth/config/verify shape loadable.
- [x] If it does not ship: the finding lands in `docs/designs/` and this story closes `done` with the
      finding as its deliverable, not `blocked`.
      → `docs/designs/graphql-vendors.md`. **This story should close `done`**, per this item — the
      finding is the deliverable. Status is left `in-progress` only because closing it runs
      `/track:done`, which writes the CHANGELOG and the board, both coordinator-owned.

## Progress
- **Refused, with the connector withdrawn. 2026-07-31, round 2.**

Round 1 shipped an eight-operation `providers/linear.toml` that emitted cleanly and passed the whole
scoped gate. Review found it could not make a single call, and the defect is upstream of everything
that gate covered. The connector is withdrawn; the measurement is the deliverable.

### The blocking finding

`connector-pack` derives an operation's configuration variables by scanning **every string literal in
the emitted body for `{…}`** (`crates/connector-pack/src/request.rs`, `endpoint_variables`). A
GraphQL query document is a string literal full of braces, none of which is configuration. That
module documents the invariant this breaks — brace-bearing literals "are of exactly two kinds, and
both are configuration" — and a query document is a third.

Both outcomes are wrong, and the second is worse:

- **Unconfigured** (the production shape — no `[[config]]` field could declare a selection set):
  every operation refuses before assembling anything, naming a "variable" that is a fragment of
  GraphQL.
- **Configured**: `Build::substitute` rewrites the document. `{ viewer { … } }` is replaced by the
  host's value, so the constant query a caller must not choose is chosen after all — by whoever
  supplies the tenant's settings. That falsifies the one property the connector existed to
  demonstrate.

Measured, not argued, in `crates/connector-pack/src/request.rs`:
`a_graphql_document_in_a_literal_is_read_as_configuration_variables` and
`a_graphql_operation_cannot_be_called_and_is_corrupted_when_it_is` — the second executes the real
`build` against an **empty** configuration, which is the check whose absence let a fully green gate
coexist with eight dead operations.

### Where the fix belongs — not here

The scan is a **stand-in**, and the module says so: it infers configuration from Flux "rather than
waiting for C-87 to publish them". So the fix is [C-87](C-87-configuration-codegen.md) — publish the
configuration surface so the pack *reads* an operation's variables instead of inferring them from
syntax. Not a cleverer scan, and explicitly not `endpoint.*` fields declared to absorb the selection
sets, which would be a connector shaped around a defect. That is a `connector-pack`/catalogue
mechanism spanning three crates and a documented safety invariant (C-193's "literals only is the
safety half"); it is not a provider story.

### What the probe established, so it is not re-derived

Four of six boundaries are already expressible — path-per-operation is a non-event, C-55's constant
body field genuinely covers a query document, a multi-line document round-trips as `"""…"""`, and the
`data.<field>` envelope is declarable and *stronger* than a REST schema. Two are not: `risk`/
`idempotency` are forced for every operation because `check_write_metadata` reads the verb
(conservative, and not on its own disqualifying), and a failed call arrives as HTTP 200 with nothing
able to say so ([C-57](C-57-quirks-beyond-http-shape.md)). All six are pinned in
`crates/connector-flux/tests/linear_connector.rs` against a fixture, in the shape
[C-164](C-164-provider-algolia.md) used for Algolia.

### Why round 1's gate was green, which is the part worth keeping

`every_shipped_operation_builds_an_absolute_request` asserts only on the **URL**, and its
`configuration()` helper manufactures a value for every *discovered* variable — so it fabricates
exactly the values that hide the refusal. `every_shipped_configuration_variable_is_placed` (C-214)
**would** have caught it at integration, because a GraphQL fragment gets no request position; that
check landed after round 1's base, which is why round 1 saw nothing. It is pinned here so the
coincidence does not have to hold next time.

There is also a structural blind spot worth its own story: **a new provider's operations are not in
the catalogue index until the coordinator regenerates it**, and the index is coordinator-owned — so
no test a provider story can run reaches `connector-pack` with its own connector in it.

### Follow-up worth filing
- **A provider story cannot test its own connector through the host library.** The blind spot above.
  Any future provider with a genuinely novel shape has it.
- **A `POST` that is semantically a read cannot say so.** Not a loosening of `check_write_metadata`;
  the IR has no way to state that a verb is a transport detail.
- **C-57 has a second, sharper consumer.** Slack's `{"ok": false}` is the filed case; a GraphQL
  vendor is the case where *no* call is status-signalled.

## Notes
- The constant-query shape leans on the same mechanism as a constant body field, which the emitter
  already reads from a JSON Schema `const` — read `crates/connector-flux/src/op.rs`'s `constant`
  before designing anything new.
- Response schemas are the weak point: a GraphQL response is shaped by its query, which is the one
  place this model should be *stronger* than REST, since the query is fixed at build time.
