---
id: C-136
title: "A credential-producing operation returns a handle, never the secret"
pillar: Spec
status: done
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
- `connector_flux::Error::CredentialProducingOperation` — **the module path is closed, not covered.**

**The correction that mattered, recorded because the first version of this story got it wrong.** The
diversion above is a `connector-pack` mechanism, and this repository has a *second* execution
surface: the emitted `connectors/<provider>.flux` module a flux runtime lifts and runs. There is no
diversion there and there cannot be — an emitted `op` ends `response = http.request(…)` /
`return response` and Flux holds no handle on the credential store, so a module carrying a login
would perform it and bind the raw token to a model-visible symbol. `AGENTS.md` § Authentication
contract has forbidden exactly that since long before this epic. The first version of this diff left
that path silent while `docs/designs/authentication-surface.md` and `crates/connector-pack/src/mint.rs`
both claimed "on **any** path" — a claim a command contradicts, which § Before you assert anything
names as its own defect class.

`check_credential_diversion` (`crates/connector-flux/src/op.rs`, beside `check_write_metadata` and
called from the same `lower`) now refuses any operation declaring `produces_credential`, so such a
connector does not build. Two alternatives were rejected rather than not considered: teaching the
emitter is forbidden by the invariant and unimplementable anyway; and "emit into the catalogue but
withhold from the module" is not available, because `emit_operation` produces **one** rendering that
`connector-cli`'s seam feeds to the module, the per-operation `.flux` and `web/public/catalog.json`
alike — deliberately, so their agreement is a property rather than a coincidence. Splitting it would
publish a login's Flux in the public catalogue anyway and break three coherence checks. Refusing
states the true thing: **this execution format cannot express a credential-producing operation.**
`an_operation_that_mints_a_credential_is_refused_rather_than_emitted` pins it, with the unmodified
operation's emitted body as the control — that text is what a login would otherwise have shipped.

**What a follow-up owes.** Un-withholding is *not* automatic, and the count is four plus one rather
than five. The **four** C-430 withheld — `zoom-meeting-get`, `zoom-meeting-create`,
`postmark-server-get`, `postmark-server-list` — return a credential *incidentally*, beside the
meeting or the server that is the operation's actual result, so diverting the field would delete the
answer rather than the exposure. They are [C-79](C-79-sensitive-response-fields.md)'s and must not be
reinstated on the strength of this story. The **one** this mechanism is shaped for is babelforce's
`POST /oauth/token`, withheld separately in v0.9.0 — and it is blocked by a *second*, independent
rule this story does not touch: `AGENTS.md` § Authentication contract states that an authentication
endpoint is never a connector operation at all. See **Open question** below.

**Known limit, stated rather than discovered.** The `ToolSpec` a model receives still carries
`output_schema: None`, because the projection derives it from the emitted Flux `op`'s return type and
every operation in this repository returns `Any`. The declared output above is carried by the IR and
by `catalog.json`; the *effective* output is the handle on every path. Narrowing the `ToolSpec` is a
change to the emitter and moves every artifact.

**The published-API break is two crates, not one.** `catalog::Acquisition` gains a variant and
`connector_spec::ir::Operation` gains a public field; neither is `#[non_exhaustive]`, and
`codewandler-connector-catalog` and `codewandler-connector-spec` are both in the publish closure.
Pre-1.0 that is a **minor** bump per `AGENTS.md` § Release process. C-430 set the precedent for the
`Operation` half — nine in-tree fixtures needed `produces_credential: None` for the same reason
`credential_response` needed one.

## Open question — the trigger, not the diversion

**The diversion is settled. What performs it is not, and this story cannot settle it alone.**

`AGENTS.md` § Authentication contract says, owner-stated, that **an authentication endpoint is never
a connector operation**: `/oauth/token` and its equivalents describe *how to authenticate*, that is a
property of the connector's authentication surface (`OAuth2Spec`, the grant, the redirect), and it is
what the **host** performs. The same section calls the credential-response rule a *"second,
independent test"* — so landing C-136 clears one of two gates and leaves the other untouched. This
epic's design meanwhile asks for `oauth2.login(grant: password, …)` as a triggerable member, which is
on the wrong side of that rule.

**My reading, for the owner.** Keep the rule; move the trigger, not the diversion.

Read the owner's own three reasons. `authorize` is a browser redirect with no result to return to a
program. `revoke` takes a `client_secret` as a plain operation argument. `token` was given *two*
reasons — it is authentication-surface material, **and** its response body is a credential. C-136
answers only the second. The first is the load-bearing one and it is unchanged: `AGENTS.md:453-456`
already says the host performs effectful acquisition such as OAuth2.

So the mechanism this story built should attach to **`OAuth2Spec`**, the declaration that already
says how a host authenticates, rather than to a new operation. Nothing in the diversion resists that:
`catalog::Acquisition::Minted { by, from }` says *"this credential is acquired by minting, and here
is what performs it"*. Today `by` is an operation id. Pointed at a declared grant instead, the
acquisition would run where `connector-pack` already resolves credentials —
`Credentials::resolve`, on a `NotFound` — and:

- the **host** performs the acquisition, which is what the contract already requires;
- no authentication endpoint becomes an operation, so the owner's rule stands unamended;
- **no Flux is emitted for it at all**, which dissolves the module problem below rather than
  managing it;
- the diversion, the handle, the store write and every refusal above are unchanged. Only the trigger
  moves.

What that costs is the epic's original ask — a login a model can *trigger*. I think that is the right
thing to lose. This design's own "What it does not protect against" says the **inputs** are still
session values: a `password` grant takes a username and a password that exist in the session before
the call. A triggerable login therefore removes half the danger and keeps the other half, while
"the host acquires, an operator configures, nothing triggers" removes both — and the design already
concludes that the category should default to operator level with a model-triggerable login as the
deliberate exception. The exception, on this reading, is one nobody has yet justified.

Until the owner rules, **the operation-shaped arrangement is refused at emission** — see below.
