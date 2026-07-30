---
id: C-4
title: Ingest OpenAPI 3.x into the IR
pillar: Spec
status: ready
priority: 5
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec]
---

# Ingest OpenAPI 3.x into the IR

## Goal
Turn a vendored vendor OpenAPI document into IR operations — servers, paths, methods, parameters, and
schemas — so a provider TOML shrinks to a pointer plus patches.

## Acceptance
- [ ] OpenAPI 3.0 and 3.1 documents parse into `Connector` operations with path, query, header, and
      body parameters, each carrying its resolved JSON Schema.
- [ ] `$ref` resolution within the document works, including nested and repeated refs; a cyclic ref
      is reported as an error rather than hanging.
- [ ] `servers` produce the base URL, with templated server variables preserved for tenant
      substitution (e.g. Zendesk's per-account subdomain).
- [ ] Missing or malformed sections degrade to a reported diagnostic naming the offending path — a
      real vendor spec is never fully well-formed, and one bad endpoint must not fail the whole
      ingest.
- [ ] Fixture-driven tests over trimmed real Zendesk and Anthropic spec excerpts committed under
      `specs/`.

## Progress
- (not started)

## Notes
- Ingest takes bytes; fetching is `C-14`'s job.
- Do not attempt to expose every endpoint — selection is `C-6`. Ingest's job is to make everything
  *available* to patch.
- Real vendor specs are frequently wrong or incomplete; the diagnostics path is the important half of
  this story, not the happy path.
