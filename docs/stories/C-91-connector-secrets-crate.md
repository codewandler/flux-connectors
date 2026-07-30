---
id: C-91
title: "`connector-secrets` — the store trait and a Vault implementation"
pillar: Bridge
status: in-progress
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
- [x] A new `crates/connector-secrets`, and **`connector-cli` does not depend on it** — asserted by a
      test over the dependency graph, not by convention. This is what keeps
      `crates/connector-cli/tests/no_network.rs` a true statement about the build.
- [x] `SecretStore` with `get` / `put` / `delete`, each returning a typed error. The three gaps in
      flux's trait, closed deliberately:
      - **`delete` exists.** flux's trait has none, so `flux auth set --clear` bypasses it entirely
        and cannot clear a Vault-backed credential.
      - **`get` distinguishes "not stored" from "backend unreachable".** flux's `load` returns
        `Option` and swallows transport errors with `.ok()?`, so a Vault outage is indistinguishable
        from an unconfigured integration — which a multi-tenant deployment will want to tell apart.
      - **The tenant is in the reference**, not baked into the store instance.
- [x] A `Secret` newtype carrying the value with a non-leaking `Debug` and **no `Serialize`**,
      mirroring `flux_secret::Material`'s compile-time guarantee that it cannot reach a model.
- [x] `VaultStore` behind a `vault` feature, KV **v2**. Record the version choice: action-proxy's
      existing paths are KV v1, so any future import is a migration rather than a rename.
- [x] The store composes with a `Layout` rather than hard-coding one — the decorator this epic exists
      for. A test proves a non-default layout changes the path and nothing else.
- [x] No expiry, no refresh, no rotation, no revocation. Out of scope by instruction, and flux already
      owns that machinery.
- [x] Tests run without a live Vault. A `MemoryStore` implementation is the fixture; the Vault client
      is exercised against a recorded transcript or a local dev server, and the story says which.

## Progress

Landed. `crates/connector-secrets` — `SecretStore`, `Secret`, `MemoryStore`, and `VaultStore` over
KV v2 behind the `vault` feature.

**The fence is `crates/connector-cli/tests/dependency_fence.rs`**, and it is the load-bearing part
of this story. It parses **`Cargo.lock`** — not `cargo metadata` — and walks the transitive closure
of `connector-cli`, `connector-spec`, `connector-flux` and `connector-catalog`. Two properties are
deliberate and must survive future tidying:

- The **lock records optional dependencies**, so the fence trips even when the edge is added behind
  a feature flag, which is how this invariant would realistically be broken. `cargo metadata`
  without `--all-features` would not see it.
- It asserts `connector-secrets` **exists** before asserting nothing reaches it, so it cannot pass
  vacuously once someone renames or removes the crate.

It has been seen to bite. Adding `connector-secrets.workspace = true` to
`crates/connector-cli/Cargo.toml` fails it with `connector-cli -> connector-secrets`; the edit was
reverted. The transitive case is asserted over a synthetic graph in the same file
(`the_walk_finds_an_edge_that_is_not_direct`), because proving it for real would have meant editing
another story's manifest.

**What C-116 should call**, rather than defining a second port:

```rust
#[async_trait::async_trait]
pub trait SecretStore: Send + Sync {
    async fn get(&self, reference: &CredentialRef) -> Result<Secret, StoreError>;
    async fn put(&self, reference: &CredentialRef, secret: &Secret) -> Result<(), StoreError>;
    async fn delete(&self, reference: &CredentialRef) -> Result<(), StoreError>;
}
```

Object-safe, so `Arc<dyn SecretStore>` is exactly C-116's "bound when the pack is constructed, not
looked up globally". `async` because `flux_runtime::Tool::execute` is
(`crates/connector-pack/src/tool.rs:47`), so a synchronous trait would put a blocking call inside a
Tool. Addressing is C-90's, **re-exported and not redefined**:
`connector_secrets::{CredentialRef, Layout, TenantLayout, validate_tenant}` all come from
`connector_spec::credential`. `connector-pack` may depend on this crate — the fence is about the
compiler crates, not about the host-facing pack.

For C-116's "a missing credential is a clear, actionable error naming the `CredentialRef` that was
not found": use `StoreError::is_not_found()`. Every variant carries the **rendered path** rather
than the reference, because a reference renders differently under each `Layout` and quoting the
address alone would send an operator to the wrong place.

**Vault: KV v2, and the transport is a seam.** `VaultStore<T: VaultTransport, L: Layout>` owns
everything that is *Vault* — `/v1/<mount>/data/<path>` to read and write,
`/v1/<mount>/metadata/<path>` to delete so that `--clear` removes every version rather than
soft-deleting the newest, the `data.data` envelope, and the status mapping. All of it is tested
**against a recorded transcript, offline — not a live server**: Vault's own KV v2 response bodies as
literals in `src/vault.rs`. That is what makes these assertions rather than hopes:

- a soft-deleted version (`200` with `data.data: null`) is `NotFound`, not an empty secret;
- a sealed Vault (`503`) is `Unreachable`, not `NotFound` — otherwise an outage looks like every
  tenant disconnecting at once;
- a **KV v1** mount is named as such rather than misread, which matters because action-proxy's
  existing paths are v1, so importing them is a **migration, not a rename**: v2 differs in the URL,
  the envelope, delete semantics, and cannot share a mount with v1;
- `403` is `Denied` and a transport failure is `Unreachable`, which is the distinction flux's
  `Option`-returning `load` cannot make.

The remaining ~60 lines are the reqwest `HttpTransport`. `tests/vault_live.rs` exercises it against
a real dev server when `CONNECTOR_SECRETS_VAULT_ADDR` / `CONNECTOR_SECRETS_VAULT_TOKEN` are set and
**skips with an explanation** otherwise. It is not `#[ignore]`d: an ignored test reports nothing,
and there is no path here where it reports success without having talked to something.

**What the default gate does and does not prove.** `vault` is **off by default**, so
`cargo build/test/clippy --workspace` does *not* compile `VaultStore`, `HttpTransport` or their
tests. That is deliberate — the compiler crates link no HTTP client, and making every story's
workspace build compile rustls and hyper would be a real tax. The vault legs must be run explicitly:

```bash
cargo test   -p connector-secrets --features vault
cargo clippy -p connector-secrets --all-targets --features vault -- -D warnings
```

**Out of scope and staying that way:** no expiry, no refresh, no rotation, no revocation, and no
session handling. Static token only. flux's `VaultCredentialStore` already does Kubernetes auth, a
60s renew buffer, `renew-self` and one retry on 401/403 — including the
re-read-the-projected-JWT-on-every-login fix that three codebases here have each had to learn.
Reimplementing any of that badly would be worse than not having it.

- The addressing layer landed 2026-07-30 with [C-90](C-90-credential-addressing-epic.md).

## Notes
- Do not reimplement flux's Vault session handling. `VaultCredentialStore` already does static-token
  **and** Kubernetes auth with a 60s renew buffer, `renew-self`, and one retry on 401/403 — including
  the re-read-the-projected-JWT-every-login fix that three codebases in this ecosystem have each had
  to learn (kubelet rotates it roughly hourly).
- The obvious cheap alternative — `impl flux_credentials::CredentialStore` and stop — is
  [C-93](C-93-flux-credential-store-adapter.md), and it is complementary rather than competing.
