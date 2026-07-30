---
id: C-107
title: Ship the Notion connector
pillar: Spec
status: ready
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "forces C-55 — Notion REJECTS a request without a Notion-Version header, so this connector cannot ship until a provider can declare a constant request header"
---

# Ship the Notion connector

## Goal
Pages, databases and search — and the connector that makes a `ready` story unavoidable.

## Acceptance
- [ ] A curated operation set: retrieve a page, query a database, create a page, search.
- [ ] **`Notion-Version` is sent on every request.** Notion rejects a request without it, so this is
      not a nicety — it is the connector working at all.
- [ ] Auth: a bearer integration token.
- [ ] A `[[config]]` surface and a `verify` operation.
- [ ] A per-provider contract test asserting the version header reaches every emitted operation.

## Progress
- Not started.

## Notes
- **Ordering edge: this depends on [C-55](C-55-constant-request-headers.md)** (*"Let a provider
  declare a constant request header"*, `status: ready`, unimplemented). There is no way to send a
  constant header today. Either C-55 lands first, or this story absorbs it — and if it absorbs it,
  it is no longer disjoint from anything else touching the emitter and must run solo.
- The alternative — declaring the version as a required parameter every caller passes — is wrong and
  should be refused: it is a constant of the connector, not an input, and a model would have to guess
  it on every call.
