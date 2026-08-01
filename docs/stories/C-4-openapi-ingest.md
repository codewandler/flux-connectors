---
id: C-4
title: Ingest OpenAPI 3.x into the IR
pillar: Spec
status: ready
priority: 1
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec]
note: "the trunk of the spec front-end — seam.rs:160 refuses every spec-backed provider until this lands, so the `[spec]` schema landed with C-3 has been unused ever since"
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
- [ ] **YAML as well as JSON.** Every babelforce document is YAML, and the spec cache is already
      extension-agnostic (`discover_specs` takes the version from the file stem). `serde_norway` is
      pre-added to `crates/connector-spec/Cargo.toml` by the coordinator — do not add or change any
      dependency yourself.
- [ ] **`crates/connector-cli/src/seam.rs:160`'s refusal is deleted**, not worked around. A
      failing-first test builds a provider whose `[spec]` points at a fixture and asserts operations
      reach the IR; today it fails with "spec ingest (story C-4), which is not wired yet".
- [ ] Ingest is a pure function from bytes to IR — `connector-spec` must not touch the network
      (`AGENTS.md`, Ownership boundaries).

## Progress
- (not started)

## Notes
- Ingest takes bytes; fetching is `C-14`'s job.
- **Ingest makes everything available; it selects nothing.** With no patch, a spec-backed provider
  still yields no operations — selection is opt-in and is C-6/C-411's job. Prove that with a test
  rather than leaving it to inference.
- Scale to design against, measured on the babelforce documents: 398 operations, 848 component
  schemas, 527 parameters of which 47 reach their definition through a `$ref`. Nested and repeated
  refs are the common case here, not the corner.
- Do not attempt to expose every endpoint — selection is `C-6`. Ingest's job is to make everything
  *available* to patch.
- Real vendor specs are frequently wrong or incomplete; the diagnostics path is the important half of
  this story, not the happy path.
