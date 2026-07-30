---
id: C-67
title: Declare the scopes an operation requires
pillar: Spec
status: ready
priority: 6
design:
epic: connectors-v1
areas: [connector-spec, connector-cli]
note: least privilege, and mechanical 403 diagnosis
---

# Declare the scopes an operation requires

## Goal
Let an operation say which permissions its credential needs — `chat:write`, `repo`,
`tickets:write` — so a connector can state least privilege, a tool catalogue can hide what the
token cannot call, and a 403 is diagnosable from the manifest instead of by experiment.

## Acceptance
- [ ] An operation declares required scopes, attached to a **credential requirement** rather than
      floating free: a requirement set already says *which* credential, and scopes say *what that
      credential must be allowed to do*. An AND-set of credentials carries scopes per credential.
- [ ] Scopes are **declarative only and grant nothing.** A test asserts no scope field can influence
      what reaches a request; this must not become a second, ungated auth path beside the `$auth`
      seam (C-10).
- [ ] Extractable in principle from what vendors publish — OpenAPI's
      `security: [{oauth2: [scope, …]}]`, Slack's per-method bot-scope table, GitHub's fine-grained
      permission list — and the design records that mapping even though authoring is by hand until
      C-4 lands.
- [ ] The connector manifest and the public catalogue publish an operation's required scopes, and
      the **union per service** so an operator can provision one token deliberately rather than by
      trial.
- [ ] Declared on the operations that already ship for at least Slack and GitHub, where the vendor
      documents scopes per method: a test asserts every operation of those providers declares its
      scopes, so a new operation cannot be added without them.
- [ ] Generated docs (C-31) show an operation's scopes beside its credential.

## Progress
- Not started. Filed 2026-07-30 while answering "what else could a connector carry".

## Notes
- **The strongest candidate of that list, for one reason:** it is the only missing property that
  changes what a *model* is allowed to attempt. Risk and idempotency describe consequence; scopes
  describe permission, and a tool catalogue that offers an operation the token cannot call spends a
  turn to learn what the manifest already knew.
- Composes with C-19's axes without touching them: a scope is not a source, an acquisition, or a
  placement — it is a property of the *grant*, which is why it belongs on the requirement rather than
  on the credential.
- Sibling property deliberately left unfiled: an operation's **cost** hint. OpenAI made `risk`
  carry that meaning by proxy (C-51), which works for now and would need real per-token pricing to
  do better.
