---
id: C-110
title: Ship the Linear connector — or record why a GraphQL vendor cannot be one
pillar: Spec
status: ready
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
- [ ] Either a working Linear connector, **or** a written finding that the operation model cannot
      describe a GraphQL vendor honestly, with the specific reason. Both are acceptable outcomes and
      the story is not a failure if it ends in the second.
- [ ] If it ships: operations are `POST /graphql` with a **constant** query in the body and the
      variables as parameters — so each operation still has a real typed signature and a real
      response schema, rather than one `query: String` parameter a model has to author.
- [ ] **Refuse the tempting wrong shape.** A single `linear-graphql(query)` operation that takes an
      arbitrary query string is exactly the remote-expression-evaluator the vision rejects: it hands
      a model a language instead of an operation, and nothing about it is typed, curated or
      analyzable.
- [ ] If it ships: auth is a bearer API key; a `[[config]]` surface; a `verify` operation; a
      per-provider contract test.
- [ ] If it does not ship: the finding lands in `docs/designs/` and this story closes `done` with the
      finding as its deliverable, not `blocked`.

## Progress
- Not started.

## Notes
- The constant-query shape leans on the same mechanism as a constant body field, which the emitter
  already reads from a JSON Schema `const` — read `crates/connector-flux/src/op.rs`'s `constant`
  before designing anything new.
- Response schemas are the weak point: a GraphQL response is shaped by its query, which is the one
  place this model should be *stronger* than REST, since the query is fixed at build time.
