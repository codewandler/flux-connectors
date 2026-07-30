---
id: C-9
title: Emit request bodies, headers, and response handling
pillar: Codegen
status: backlog
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-flux]
---

# Emit request bodies, headers, and response handling

## Goal
Extend the emitter past read-only GETs: JSON request bodies, static and parameterized headers, and
turning an HTTP response into a useful op result including the vendor's error envelope.

## Acceptance
- [ ] POST/PUT/PATCH operations emit a JSON body assembled from IR body params.
- [ ] Static headers (e.g. `content-type`) and parameterized headers emit correctly.
- [ ] A non-2xx response becomes a **structured result**, matching `http.request`'s contract that
      non-2xx is a result rather than an op failure, with the vendor's error envelope surfaced.
- [ ] Write operations declare non-idempotent `idempotency` and an honest `risk`, so flux's approval
      gate treats them correctly.
- [ ] Golden-file tests for a POST with a body and for an error-envelope response.

## Progress
- (not started)

## Notes
- `http.request` returns status, headers, and a byte-capped body
  (`../flux/crates/flux-web/src/http.rs`); 256 KiB cap, cut on a char boundary.
- Response *shaping* (projecting large payloads down to fit a context budget) is deliberately out of
  scope here and deferred past milestone 1 — see the pipeline design's open questions.
