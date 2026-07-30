---
id: C-9
title: Emit request bodies, headers, and response handling
pillar: Codegen
status: in-progress
priority:
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-flux]
---

# Emit request bodies, headers, and response handling

## Goal
Extend the emitter past read-only GETs: JSON request bodies, static and parameterized headers, and
turning an HTTP response into a useful op result including the vendor's error envelope.

## Acceptance
- [x] POST/PUT/PATCH operations emit a JSON body assembled from IR body params.
- [x] Static headers (e.g. `content-type`) and parameterized headers emit correctly.
- [x] A non-2xx response becomes a **structured result**, matching `http.request`'s contract that
      non-2xx is a result rather than an op failure, with the vendor's error envelope surfaced.
- [x] Write operations declare non-idempotent `idempotency` and an honest `risk`, so flux's approval
      gate treats them correctly.
- [x] Golden-file tests for a POST with a body and for an error-envelope response.

## Progress

**Done (C-9).** `crates/connector-flux/src/op.rs` now emits bodies, headers and an explicit response
bind + `return`. Two new goldens (`freshdesk-ticket-note-add.flux`, `zendesk-ticket-show.flux`); the
four C-8 goldens each changed by one line, `do http.request {…}` → `$response = http.request({…})` +
`return $response`.

Four decisions a later story should not have to re-derive:

- **The body is bound, then passed by symbol.** `http.request` reads `body` with `Value::as_str`
  (`../flux/crates/flux-web/src/http.rs:183-186`), so an *inline* record arrives as a JSON object and
  is dropped silently. A bound record is stored as canonical JSON text and arrives intact.
- **Every literal the emitter contributes is bound to a symbol** (`$content_type`, and each constant
  body field). A record whose values are all literals is not "dynamic" to flux-lang's AST formatter,
  which spells it `@json`; flux's CST formatter then re-spaces it and the module stops being a fixed
  point of the formatter.
- **A write may not carry a read's metadata**, and this is enforced by refusal rather than by a
  silent correction: `risk = "low"` on POST/PUT/PATCH/DELETE, and `idempotency = "idempotent"` on
  POST/PATCH (RFC 9110 §9.2.2 — PUT and DELETE genuinely are idempotent and are left alone).
- **A JSON Schema `const` on a body field means "always sent, never caller-supplied".** Zendesk's
  `ticket.safe_update` is emitted into the payload and kept out of the op signature.

**Open, and blocking real Zendesk writes — needs an additive field on `connector_spec::Param`.**
`ParamSet::body` is a flat `Vec<Param>` with one `name`, so a nested wire body has no
representation. Where it is detectable the emitter refuses (`Error::NestedBodyField`, which is what
`babelforce-agent-status-update`'s `presence.name` now hits). Where it is **not** — Zendesk records
the caller-facing name in `name` and the wire path in the parameter's *description* — the three
`zendesk-ticket-*` writes emit a flat body the vendor will ignore. The fix is one field, e.g.
`Param::wire: Option<String>` (a dot-separated body path; it would also carry Freshdesk's
`req_id` → `requester_id` alias). Two babelforce operations additionally need
`ParamSet::body_schema: Option<JsonSchema>` for a free-form object body.

**Deferred, unchanged:** the vendor's error envelope is surfaced on the op *description*, not dug out
in Flux — `http.request` returns one flat `HTTP …\n…\n…` string, and neither `jq` nor an `expr`
split can recover the body from it canonically. See `op.rs::description` for the full reasoning; a
record-returning `http.request` is a seam story on flux.

## Notes
- `http.request` returns status, headers, and a byte-capped body
  (`../flux/crates/flux-web/src/http.rs`); 256 KiB cap, cut on a char boundary.
- Response *shaping* (projecting large payloads down to fit a context budget) is deliberately out of
  scope here and deferred past milestone 1 — see the pipeline design's open questions.
