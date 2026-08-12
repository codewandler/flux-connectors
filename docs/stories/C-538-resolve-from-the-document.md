---
id: C-538
title: "Resolve requests from the document, not the Flux"
pillar: Connector
status: done
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

- [x] The plan-deriving core lives in an **engine-free** crate: its `resolve` returns the request
      plan as data (secret-bearing fields on the redacted-`Debug` pattern), it carries no
      `codewandler-flux-*` dependency, and a dependency-direction test pins that (the
      `dependency_fence.rs` pattern). `connector-pack`'s existing `resolve`/`project`/`pack`
      signatures survive as a thin wrapper over the core, so no consumer breaks; the wrapper's
      retirement belongs to Exchange's plan-API adoption (X-151), not this story.
      *(`codewandler-connector-resolve`, lib `connector_resolve`; `RequestPlan { request,
      permission_subjects, redactions: Vec<SensitiveText> }`; the fence is
      `engine_free_core.rs` with a control proving the engine IS elsewhere in the graph.)*
- [x] `build_request` reads the request template instead of walking a parsed module; `spec.rs`'s
      `parse_str` and `request.rs`'s AST evaluation are unreachable from the resolve path.
      **Met for the request/resolve path; a recorded residue remains at the *projection*:** the
      `ToolSpec` a model receives still parses emitted Flux, because the document does not carry
      the error-envelope-extended description or the contract `input_schema`, and reproducing
      those emitter rules without gate coverage was judged worse than the parse. The closure —
      which is also what C-540's deletion actually waits on — is C-552.
- [x] **The differential gate:** a workspace test proves, for every operation in the catalogue,
      that the document-derived plan is byte-identical to the Flux-derived plan — method, URL,
      headers, query, body, `permission_subjects`, and the registered redaction set — **and** that
      the document-backed configuration surface (endpoint variables, slots, caller path
      parameters) agrees with the Flux-derived `Rehearsal`'s, because Exchange's settings and
      connection-verification paths consume that surface. The gate lands failing-first against a
      seeded divergence and runs in CI until C-540 deletes the old derivation.
      *(`catalogue_differential.rs`: 835 compared, count asserted against the catalogue; red at
      base on a real class — the document publishes IR names, callers address Flux symbols, 23
      operations — plus two seeded-divergence controls; 1.52 s debug wall clock.)*
- [x] Every `connector_pack::Error` variant keeps its name and its trigger; the auth assembly,
      checked redactor registration, endpoint substitution with declared-authority validation, and
      channel plans are untouched in behaviour (their existing tests keep passing unmodified).
      *(One recorded exception: `configuration_value_guard.rs`'s header-pin fixture switched its
      injection vector from doctored Flux — now inert — to a doctored document; assertions
      unchanged, the `Slot::Header` guard moved crates intact with its own unit test.)*
- [x] `Rehearsal` is replaced by a document-backed equivalent with the same observable semantics,
      kept exported until the Exchange call sites in
      `../flux-exchange/crates/exchange-host/src/settings.rs` migrate (C-539).
      *(`DocumentRehearsal`, exported beside `Rehearsal`, same surface.)*
- [x] `expose`, endpoint variables, endpoint slots and caller path parameters — everything the
      pack currently recovers from the module text — come from document fields.

## Progress

- 2026-08-12 — Implemented on `impl/C-538` (`3b0e7a6f`), merged at integration; independent
  review passed before the merge. Delivered as VERDICT: PARTIAL by its implementor for the one
  honest residue recorded in the second acceptance bullet, integrated with that residue filed as
  C-552 rather than bounced: the alternative was reproducing emitter rules no gate holds honest.
  Two safety findings from implementation, both resolved in the diff: the whole-catalogue gate's
  base-red exposed the IR-name/Flux-symbol divergence (now reproduced by
  `connector-resolve`'s allocator and pinned by the gate for all 835 operations — but only while
  the emitter exists, which is C-552/C-540's dependency), and `live_egress.rs`'s Flux-doctoring
  retarget went inert and sent four real requests to `api.openai.com` (sentinel key, 401s) before
  the retarget moved into the `Egress` transport with a no-op-refusal assertion. Peak risks
  carried in the handoff: the symbol reproduction, the lazy `Mutex`+leak document cache, and the
  const-pinned-param allocator trap (C-552's acceptance names it).

## Notes

- Depends on C-536 (documents) and C-537 (reader). Write set is `crates/connector-pack`; do not
  share a wave with a story writing it.
- The differential gate is the migration rule of Decision 0022 made executable; it is the single
  piece of evidence that authorises every deletion in C-540.
