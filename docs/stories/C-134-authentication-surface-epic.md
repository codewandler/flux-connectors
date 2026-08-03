---
id: C-134
title: "Authentication as a connector surface — a login that cannot leak (epic)"
pillar: Spec
status: ready
priority: 2
design: docs/designs/authentication-surface.md
epic: authentication-surface
areas: [connector-spec, providers, bridge]
note: "EPIC — an operation's result becomes a session value the model can read, so a login that RETURNS its token hands a bearer credential to an LLM. The answer is structural: divert to the store, return a CredentialRef. Redaction cannot work here — C-79 already proves why"
---

# Authentication as a connector surface — a login that cannot leak (epic)

## Goal

Let a connector declare a **triggerable** login — `oauth2.login(grant: password, …)` and its sibling
grants — as members of an `authentication` service, without any path by which the minted credential
becomes readable.

## Acceptance

- [ ] `authentication` is a **role** on a service, reusing [C-119](C-119-provider-roles-epic.md)'s
      mechanism rather than inventing a category system beside it, with required members per grant —
      [C-135](C-135-authentication-role.md).
- [ ] **A credential-producing operation returns a handle, never the secret.** Its declared output is
      a [`CredentialRef`](C-90-credential-addressing.md); the token goes from the HTTP response
      straight into the bound `CredentialStore` and never enters the session —
      [C-136](C-136-credential-diversion.md).
- [ ] An operation whose declared output would expose the secret field is **refused at load**.
- [ ] The category defaults to **operator level**; a model-triggerable login is a deliberate
      exception, not the default.
- [ ] `OAuth2Spec` gains its first real consumer — [C-88](C-88-prove-oauth2.md), already filed, is
      where that lands.
- [ ] Sensitive *response* fields on ordinary operations are covered by
      [C-79](C-79-sensitive-response-fields.md), already filed. This epic generalises its mechanism
      rather than duplicating it.

## Children

- [C-135](C-135-authentication-role.md) — the `authentication` role and the grant members
- [C-136](C-136-credential-diversion.md) — **the non-exposure invariant**, and its refusals

Related and already filed: [C-79](C-79-sensitive-response-fields.md) ·
[C-88](C-88-prove-oauth2.md) · [C-91](C-91-connector-secrets-crate.md) ·
[C-116](C-116-credential-store-port.md)

## Notes

**Why redaction is not the answer, and this repo already knows.** C-79 records the concrete case:
Zoom's `start_url` carries a host-privileged token and **the redactor cannot see it**. Redaction is
string matching against values it was already told about; a token minted *by this very call* is
unknown to the redactor until after it has arrived. Redaction is a mitigation applied after the fact.
It cannot be a guarantee — so the guarantee has to come from the operation's declared shape.

**What this does not protect.** The *inputs* are still inputs: `grant: password` takes a username and
a password that exist in the session before the call. This keeps the minted token out; it does not
make a resource-owner password grant safe to hand a model. That is the argument for the operator
level and for preferring `client_credentials` and `authorization_code`.

**A2A and MCP are not in this epic.** C-495 now admits protocol connectors, but flux already ships
`crates/flux-a2a`, and MCP already exposes a tool catalogue. C-46 and the runtime-binding program must
decide how to reuse those surfaces without a second stale catalogue; this authentication epic does
not wait on that work.
