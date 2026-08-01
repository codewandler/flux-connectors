---
id: C-426
title: "`multipart/form-data` is inexpressible, and it is the last five operations between babelforce and full parity"
pillar: Spec
status: blocked
priority: 1
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec, connector-flux]
note: "measured by the C-411 selector 2026-08-01 — the canonical selection reaches 392 of 397, and the missing five are ALL multipart uploads ingest skips. `BodyEncoding` is `Json | Form` and has no third value. This is now the only thing standing between babelforce and manager-sdk parity"
---

# `multipart/form-data` is inexpressible, and it is the last five operations between babelforce and full parity

## Goal
Give `BodyEncoding` a `multipart` variant so a file-upload operation can be described and emitted,
closing the last gap between the babelforce connector and manager-sdk's canonical 397.

## Acceptance
- [ ] **Not done, and deliberately so — see the finding below.** `BodyEncoding` gains a multipart
      variant, and `crates/connector-spec/src/openapi.rs` stops skipping an operation whose request
      body is `multipart/form-data`. Adding the variant would let the IR describe a request no
      emitted module could perform, which the story's own second bullet forbids.
- [x] The emitter produces something a caller can actually use, or the story stops and says why.
      **This is the real risk and it must be faced, not routed around**: flux's `http.request` takes a
      body, and if it cannot express a multipart part with a filename and a content type then the IR
      can describe the operation and the module still cannot perform it. Establish that first, against
      the flux version this repo pins, and report the finding before writing an emitter.
- [x] The five babelforce operations are the acceptance set, named because ingest already names them:
      `POST /api/v2/agents/provision`, `POST /api/v2/agents/provision/validate`,
      `POST /api/v2/outbound/lists/{id}/leads/upload`, `POST /api/v2/phonebook/bulk`,
      `POST /api/v2/prompts`. All five stay allow-listed, each now carrying the *flux-side* reason
      rather than "C-426 closes this" — `crates/connector-spec/tests/babelforce_coverage.rs`
      (`Gap::Inexpressible`) and `operation_selection.rs::MULTIPART`.
- [x] **The accounting test flips rather than being edited.** It flipped, though for the auth-flow
      ruling rather than for multipart: no assertion was relaxed, and the accounting now names
      **three** categories — `388 emitted + 5 inexpressible + 4 withheld = 397` — counted from each
      allow-list entry's own `Gap` rather than by position.
- [ ] A nested body under multipart is refused rather than guessed. **Moot**: there is no multipart
      variant to nest under. `BodyEncoding::Form`'s refusal is untouched, and flux's own `parse`
      applies the same rule (`runtime.rs:4187`, "a nested field is an error").

## Progress
- **The feasibility question is answered, and the answer is no.** flux **cannot** carry a
  `multipart/form-data` body on the pinned engine line (`ENGINE_LINE = "0.46"`), so describing one in
  the IR would produce a module that fails on a real call. Three independent confirmations, read off
  the vendored `codewandler-flux-*-0.46.0` sources:
  1. `http.request`'s `body` parameter is declared `{"type": "string"}` and read with
     `params.get("body").and_then(Value::as_str)` (`flux-web-0.46.0/src/http.rs:119,229-232`). A
     structured body is not merely unsupported — it is silently dropped to *no body at all*. There is
     no part list, no per-part filename and no per-part content type anywhere in the tool's schema.
  2. `parse(_, as: _)` is the only serializer authored Flux can call, and its `as_type` is a **closed
     list the analyzer enforces**: `["f64", "i64", "bool", "json", "string", "form"]`
     (`flux-lang-0.46.0/src/analyze.rs:1815`). `parse($record, as: "multipart")` is a static analysis
     error, so it could never pass this repository's "generated Flux must parse and analyze" gate.
  3. `grep -rn multipart` over `flux-web`, `flux-lang`, `flux-core` and `flux-runtime` at 0.46.0
     returns **nothing**. No boundary generator exists, and a Flux string is UTF-8, so a file's bytes
     could not travel through one even if a boundary were hand-assembled.
- The only remaining route is hand-assembling boundary and CRLF framing with `fmt`, which is exactly
  the connector-specific DSL `AGENTS.md` refuses and the same shape as the form/query gap already
  recorded under `zendesk-ticket-search`. **The fix is a flux-side encoder** — an upstream story, the
  sibling of `L-101`, which gave `parse` its `form` value the same way this needs a `multipart` one.
- So the five stay allow-listed, with the reason rewritten from "C-426 closes this" to the finding
  above, in both `babelforce_coverage.rs` and `operation_selection.rs`.
- **The owner's auth ruling landed in this commit too** (coordinator-directed, mid-story). Three
  `/oauth/*` endpoints and `GET /api/v2/user/account` are withheld; babelforce goes 391 → 388
  operations and 948 → 943 artifacts. `no_oauth_endpoint_becomes_an_operation` is the failing-first
  test.
  - **Narrowed the same day, and `/oauth/token` is the only one affected**
    ([C-432](C-432-mark-a-response-as-carrying-a-credential.md)). The owner reversed the ruling for
    `token` specifically: it is a real request/response call a program makes and reads, unlike
    `authorize` (a browser redirect with no result to return) and `revoke` (which takes a
    `client_secret` as a plain argument). **Those two stay withheld on the original ground, and
    `GET /api/v2/user/account` is untouched** — it was withheld for carrying credentials in its
    response, a separate and still-standing rule.
  - **`/oauth/token` is nevertheless still withheld**, so this story's counts do not move. C-432
    found that the marking the reversal depends on does not exist: flux's credential boundary keys
    on `PlatformSourcing`, which is an opt-in to *refusal* rather than a permit, and it sits on the
    plugin seam this repository is not on. Restoring the operation needs a mechanism nobody has
    built — see C-432's finding and C-136's *"The owner ruled, and the refusal still stands"*.
- **`GET /api/v2/user/account` was verified against the document, not accepted on the field name.**
  Its 200 body carries `UserCustomer_customer_apis_babelforce`, which the vendor itself describes as
  *"REST API access credentials"* (`user-2026-06-25.openapi.yaml:402-415`), plus a sibling
  `stream.token` described *"Push API token"* (:417-421) that a name-scan for `accessToken` misses
  entirely. The `format: uuid` / *"unique Identifier (UUID)"* description is boilerplate copied onto
  both fields and the document contradicts it: `accessId`'s scrubbed example is a real UUID while
  `accessToken`'s is 32 undashed hex characters (:294).
- **The `auth` service and its `[[spec]]` both had to go, and they are coupled.** A declared service
  with zero operations emits an empty module and an `operations = []` manifest, which
  `services.rs:415` refuses; and the loader refuses a `[[spec]]` naming a service no `[[services]]`
  entry declares. Drift detection on the document is **not** lost — `specs/babelforce.provenance.toml`
  carries its `sha256`/`upstream_sha256` and `vendored_specs.rs` checks them against the bytes,
  independently of the provider file. Its three endpoints stay inside the accounting by name through
  `UNREAD` in `babelforce_coverage.rs`, so nothing vanished from both sides of the gate.

## Notes
- **This is the goal's last blocker.** The owner's standing instruction is the full manager-sdk
  surface; C-417 can deliver 392 of 397 today with these five allow-listed, and only this story closes
  the remainder.
- The gap has been known since the epic was planned — `docs/designs/spec-front-end.md` names multipart
  first under "What retiring manager-sdk actually requires" — but it was 5 of 398 in a design document
  and is now the whole distance between a connector and its parity claim.
- Sequenced **after** [C-417](C-417-widen-to-full-coverage.md), which lands the 392 and the allow-list.
  Landing this first would mean regenerating babelforce twice.
- If the answer turns out to be "flux cannot carry a multipart body", that is a legitimate outcome:
  record it, keep the five allow-listed with the reason, and file the upstream story. A connector that
  says honestly what it cannot do beats one that emits a module that fails at runtime.
