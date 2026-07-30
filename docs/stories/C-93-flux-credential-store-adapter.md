---
id: C-93
title: The flux adapter — a tenant-scoped store behind flux's CredentialStore
pillar: Bridge
status: ready
priority: 5
design: docs/designs/credential-addressing.md
epic: credential-addressing
areas: [connector-secrets, bridge]
note: "the trap: flux's CLI write path is hard-wired to the file backend, so an injected store is READ-ONLY in practice until flux changes. Say so before anyone deploys it"
---

# The flux adapter — a tenant-scoped store behind flux's CredentialStore

## Goal
Let a tenant-scoped store actually serve a running flux, through the one seam flux already has —
`SystemHostCaps::with_credential_store`.

## Acceptance
- [ ] An adapter presenting a [C-91](C-91-connector-secrets-crate.md) `SecretStore` as
      `flux_credentials::CredentialStore`, with the tenant fixed per instance. This is precisely the
      deployment shape D-83 and D-130 were built for.
- [ ] The **key translation is explicit and tested**. flux addresses `plugin:<name>:<purpose>`; this
      epic addresses `tenants/<t>/<authority>/<credential>`. The adapter maps between them, and the
      mapping is the interesting part — a test pins it in both directions.
- [ ] **The read-only trap is documented before anyone deploys it.** flux's CLI write path
      (`flux_credentials::save_token` / `delete_token`, used by `flux auth set` and `flux auth login`)
      is hard-wired to the file backend and does **not** go through the trait. So an injected store is
      read at call time and never written by the CLI: the two halves do not meet. State this in the
      adapter's own docs, not only here.
- [ ] `SystemHostCaps` is **session-lived, built once at startup**, so "a different store per request"
      does not fit flux's object lifecycle today. Record what that means for a multi-tenant deployment
      — one host per tenant, or a change to flux — rather than leaving an implementer to discover it.
- [ ] A test proves a `load` on the adapter cannot silently succeed against the wrong tenant.

## Progress
- Not started. Depends on [C-91](C-91-connector-secrets-crate.md).

## Notes
- **The flux-side gaps belong on flux's board**, filed as a handoff per the C-16 / C-64 / C-84
  precedent: D-83's dropped `[+account]`, a `delete` on the trait, and a `load` that distinguishes a
  backend outage from an unconfigured integration. This story adapts around them; it does not fix them.
- flux's `VaultCredentialStore` has zero callers today. If a deployment only needs Vault with a
  per-tenant prefix and no `delete`, that store plus a prefix is the cheaper answer than this adapter,
  and the story should say so rather than assuming the adapter is always right.
