---
id: C-136
title: "A credential-producing operation returns a handle, never the secret"
pillar: Spec
status: ready
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

- [ ] An operation may declare `produces_credential`, naming **which field of the vendor response
      holds the secret** and **which `CredentialRef` it is stored under**
      ([C-90](C-90-credential-addressing.md)'s type, which has had no consumer until now).
- [ ] Its declared output is the **handle**: `{ "credential": "tenants/<tenant>/<authority>/…" }`.
      The secret field is absent from the effective output entirely.
- [ ] **Refusals, each with its own test:**
      - a `produces_credential` operation whose declared output or `response_schema` still exposes the
        secret field → **refused at load**;
      - one that names no secret field → refused (the extractor would not know what to divert);
      - one declared `idempotent` → refused, since minting a token is a write and some vendors
        invalidate the previous one.
- [ ] The store is a **bound port**, never a global — an operation cannot mint into a store the host
      did not supply. Reuse [C-116](C-116-credential-store-port.md)'s `CredentialStore`.
- [ ] **Failing-first test:** `a_minted_credential_never_reaches_the_session` — drive a login against
      a stubbed vendor returning a sentinel token, and assert the sentinel appears in **none** of: the
      operation's result value, its `view`, an error raised on the failure path, a progress line, or
      any generated artifact. It must fail against an implementation that returns the vendor body and
      relies on redaction.
- [ ] A test asserts the **failure** path too: a login that errors after the token arrives must not
      surface it in the error.
- [ ] The gate is green; the build stays a fixed point.

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
