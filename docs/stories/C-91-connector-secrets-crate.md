---
id: C-91
title: "`connector-secrets` — the store trait and a Vault implementation"
pillar: Bridge
status: ready
priority: 3
design: docs/designs/credential-addressing.md
epic: credential-addressing
areas: [connector-secrets]
note: "a HOST LIBRARY, outside the compile path — connector-cli must not depend on it, asserted by test, so no_network.rs keeps meaning what it means"
---

# `connector-secrets` — the store trait and a Vault implementation

## Goal
Give [C-90](C-90-credential-addressing-epic.md)'s addresses somewhere to resolve, in a crate that is
explicitly a host library rather than part of the compiler.

## Acceptance
- [ ] A new `crates/connector-secrets`, and **`connector-cli` does not depend on it** — asserted by a
      test over the dependency graph, not by convention. This is what keeps
      `crates/connector-cli/tests/no_network.rs` a true statement about the build.
- [ ] `SecretStore` with `get` / `put` / `delete`, each returning a typed error. The three gaps in
      flux's trait, closed deliberately:
      - **`delete` exists.** flux's trait has none, so `flux auth set --clear` bypasses it entirely
        and cannot clear a Vault-backed credential.
      - **`get` distinguishes "not stored" from "backend unreachable".** flux's `load` returns
        `Option` and swallows transport errors with `.ok()?`, so a Vault outage is indistinguishable
        from an unconfigured integration — which a multi-tenant deployment will want to tell apart.
      - **The tenant is in the reference**, not baked into the store instance.
- [ ] A `Secret` newtype carrying the value with a non-leaking `Debug` and **no `Serialize`**,
      mirroring `flux_secret::Material`'s compile-time guarantee that it cannot reach a model.
- [ ] `VaultStore` behind a `vault` feature, KV **v2**. Record the version choice: action-proxy's
      existing paths are KV v1, so any future import is a migration rather than a rename.
- [ ] The store composes with a `Layout` rather than hard-coding one — the decorator this epic exists
      for. A test proves a non-default layout changes the path and nothing else.
- [ ] No expiry, no refresh, no rotation, no revocation. Out of scope by instruction, and flux already
      owns that machinery.
- [ ] Tests run without a live Vault. A `MemoryStore` implementation is the fixture; the Vault client
      is exercised against a recorded transcript or a local dev server, and the story says which.

## Progress
- Not started. The addressing layer landed 2026-07-30 with [C-90](C-90-credential-addressing-epic.md).

## Notes
- Do not reimplement flux's Vault session handling. `VaultCredentialStore` already does static-token
  **and** Kubernetes auth with a 60s renew buffer, `renew-self`, and one retry on 401/403 — including
  the re-read-the-projected-JWT-every-login fix that three codebases in this ecosystem have each had
  to learn (kubelet rotates it roughly hourly).
- The obvious cheap alternative — `impl flux_credentials::CredentialStore` and stop — is
  [C-93](C-93-flux-credential-store-adapter.md), and it is complementary rather than competing.
