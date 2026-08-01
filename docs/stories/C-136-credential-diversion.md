---
id: C-136
title: "A credential-producing operation returns a handle, never the secret"
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/authentication-surface.md
epic: authentication-surface
areas: [connector-spec, bridge]
note: "THE safety story of its epic. An operation's result becomes a session value a model can read and a log can print, so a login that returns its token has already lost. Redaction cannot save it — a token minted BY THIS CALL is unknown to the redactor until after it arrives"
---

# A credential-producing operation returns a handle, never the secret

## Goal

Make it **structurally impossible** for a minted credential to reach a session symbol, a model, a
log line, or an error message — by removing it from the operation's declared output rather than by
filtering it afterwards.

## Acceptance

- [x] An operation may declare `produces_credential`, naming **which field of the vendor response
      holds the secret** and **which `CredentialRef` it is stored under**
      ([C-90](C-90-credential-addressing.md)'s type, which has had no consumer until now).
- [x] Its declared output is the **handle**: `{ "credential": "tenants/<tenant>/<authority>/…" }`.
      The secret field is absent from the effective output entirely.
- [x] **Refusals, each with its own test:**
      - a `produces_credential` operation whose declared output or `response_schema` still exposes the
        secret field → **refused at load**;
      - one that names no secret field → refused (the extractor would not know what to divert);
      - one declared `idempotent` → refused, since minting a token is a write and some vendors
        invalidate the previous one.
- [x] The store is a **bound port**, never a global — an operation cannot mint into a store the host
      did not supply. Reuse [C-116](C-116-credential-store-port.md)'s `CredentialStore`.
- [x] **Failing-first test:** `a_minted_credential_never_reaches_the_session` — drive a login against
      a stubbed vendor returning a sentinel token, and assert the sentinel appears in **none** of: the
      operation's result value, its `view`, an error raised on the failure path, a progress line, or
      any generated artifact. It must fail against an implementation that returns the vendor body and
      relies on redaction.
- [x] A test asserts the **failure** path too: a login that errors after the token arrives must not
      surface it in the error.
- [x] The gate is green; the build stays a fixed point.

## Notes

- **Why redaction is not sufficient, in the concrete.**
  [C-79](C-79-sensitive-response-fields.md) records that Zoom's `start_url` carries a
  host-privileged token and **the redactor cannot see it**. Redaction is string matching against
  values it was already told about; a token minted *by this very call* is unknown to the redactor
  until after it has arrived, and by then something has already handled a response body containing
  it. Register the value with the redactor as a second line of defence — but the guarantee must come
  from the declared shape, not from the filter.
- **The property to preserve when reviewing this:** a caller can *use* a credential it can never
  *read*. Downstream operations name the ref; the host resolves it at request-assembly time. If a
  reviewer can construct any sequence of declared operations that returns the value, the story is not
  done.
- **This does not protect the inputs.** `grant: password` takes a username and a password that exist
  in the session before the call. Say so wherever a user will see it; do not let the diversion
  guarantee be read as "logins are safe to hand a model".
- Generalises C-79's mechanism from "this response field is sensitive" to "this operation's whole
  purpose is to mint one". Coordinate rather than duplicating — if C-79 lands first, build on it.

## Progress

**Landed 2026-08-01 on `impl/C-136`, branched from `3c28eaf` (Release v0.9.1).** The mechanism exists
end to end — declaration, refusals, catalogue, runtime diversion — and **no operation declares it
yet**, which is deliberate: un-withholding the four operations v0.9.0 and v0.9.1 held back is a
separate change to their provider files, and it regenerates artifacts.

**The shape, in one line each.**

- `connector_spec::ir::ProducedCredential` — `secret` (a JSON Pointer into the vendor's response
  body, C-430's own vocabulary) and `credential` (an `[[auth]]` name). On `Operation` as
  `produces_credential`, `skip_serializing_if`, so no `ir_sha256` and no artifact moved.
- `Operation::effective_response_schema()` — the **derived** declared output. For a minting operation
  it is `credential_handle_schema()`, `{ "credential": <address> }` with `additionalProperties:
  false`; for everything else it is `response_schema` unchanged. `connector-cli`'s `site` publishes
  *this*, so `web/public/catalog.json` can never offer a caller the secret.
- `provider::validate_produces_credential` — six refusals, each with its own test in
  `crates/connector-spec/tests/produces_credential.rs`. The story's three (a `response_schema` that
  still resolves the secret's location; no secret field named; `idempotency = "idempotent"`) plus the
  three without which they cannot be enforced: an undeclared credential, a connector with no
  `authority`, and two operations minting one credential.
- `catalog::Acquisition::Minted { by, from }` — how the fact reaches the runtime. A **variant**
  rather than a field on `catalog::Operation`, because a field would rewrite all 45 generated tables,
  every artifact hash under them and `connectors.lock`, for a fact nothing declares. The emitter
  joins the operation-side declaration onto the credential by name.
- `connector_pack::mint` — the diversion. `Tool::execute` splits build-and-authenticate (whose
  refusals are the pack's own and are reported unchanged) from the transport call, and **nothing
  derived from the vendor's answer leaves the second half**: the secret goes into the bound store
  through `Credentials::mint` and the caller gets the handle, or the call refuses with
  `Error::CredentialNotMinted`, which carries the operation, the credential and at most the HTTP
  status.

**What a follow-up owes.** Un-withholding is *not* automatic. babelforce's `POST /oauth/token`
becomes expressible now and is the one the mechanism was designed for. The other three are **not**
covered and must not be reinstated on the strength of this story: `zoom-meeting-get`,
`zoom-meeting-create`, `postmark-server-get` and `postmark-server-list` return a credential
*incidentally* — a meeting, a server — so diverting the field would delete the operation's actual
result. Those four are [C-79](C-79-sensitive-response-fields.md)'s, not this one's.

**Known limit, stated rather than discovered.** The `ToolSpec` a model receives still carries
`output_schema: None`, because the projection derives it from the emitted Flux `op`'s return type and
every operation in this repository returns `Any`. The declared output above is carried by the IR and
by `catalog.json`; the *effective* output is the handle on every path. Narrowing the `ToolSpec` is a
change to the emitter and moves every artifact.
