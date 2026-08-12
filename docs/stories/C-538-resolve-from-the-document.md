---
id: C-538
title: "Resolve requests from the document, not the Flux"
pillar: Connector
status: ready
priority: 1
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-pack]
note: "connector-pack derives the request plan from the request template; the parse at spec.rs:250 and the AST walk leave the resolve path behind a whole-catalogue byte-identical differential gate"
---

# Resolve requests from the document, not the Flux

## Goal

Make `connector-pack` derive its request plan from the canonical document's request template,
removing `flux_lang` parsing from the resolve path while preserving every signature, every
fail-closed refusal, and the exact bytes on the wire.

## Acceptance

- [ ] The plan-deriving core lives in an **engine-free** crate: its `resolve` returns the request
      plan as data (secret-bearing fields on the redacted-`Debug` pattern), it carries no
      `codewandler-flux-*` dependency, and a dependency-direction test pins that (the
      `dependency_fence.rs` pattern). `connector-pack`'s existing `resolve`/`project`/`pack`
      signatures survive as a thin wrapper over the core, so no consumer breaks; the wrapper's
      retirement belongs to Exchange's plan-API adoption (X-151), not this story.
- [ ] `build_request` reads the request template instead of walking a parsed module; `spec.rs`'s
      `parse_str` and `request.rs`'s AST evaluation are unreachable from the resolve path.
- [ ] **The differential gate:** a workspace test proves, for every operation in the catalogue,
      that the document-derived plan is byte-identical to the Flux-derived plan — method, URL,
      headers, query, body, `permission_subjects`, and the registered redaction set — **and** that
      the document-backed configuration surface (endpoint variables, slots, caller path
      parameters) agrees with the Flux-derived `Rehearsal`'s, because Exchange's settings and
      connection-verification paths consume that surface. The gate lands failing-first against a
      seeded divergence and runs in CI until C-540 deletes the old derivation.
- [ ] Every `connector_pack::Error` variant keeps its name and its trigger; the auth assembly,
      checked redactor registration, endpoint substitution with declared-authority validation, and
      channel plans are untouched in behaviour (their existing tests keep passing unmodified).
- [ ] `Rehearsal` is replaced by a document-backed equivalent with the same observable semantics,
      kept exported until the Exchange call sites in
      `../flux-exchange/crates/exchange-host/src/settings.rs` migrate (C-539).
- [ ] `expose`, endpoint variables, endpoint slots and caller path parameters — everything the
      pack currently recovers from the module text — come from document fields.

## Progress

- (not started)

## Notes

- Depends on C-536 (documents) and C-537 (reader). Write set is `crates/connector-pack`; do not
  share a wave with a story writing it.
- The differential gate is the migration rule of Decision 0022 made executable; it is the single
  piece of evidence that authorises every deletion in C-540.
