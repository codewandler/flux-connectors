# connector-secrets

**A host library: where a connector's `CredentialRef` resolves to a value.**

Part of [flux-connectors](https://github.com/codewandler/flux-connectors), which compiles SaaS API
descriptions into [Flux-Lang](https://github.com/codewandler/flux).

```toml
[dependencies]
connector-secrets = "0.18"
# The Vault store and its HTTP transport are opt-in:
# connector-secrets = { version = "0.18", features = ["vault"] }
```

## What it is

[`connector-address`](https://crates.io/crates/codewandler-connector-address) owns the credential
*address* —
`tenants/<tenant>/<authority>[/@instances/<uuid>][/<service>]/<credential>`, pure and validated.
The instance names which of a tenant's connections, and is carried only when it holds more than one,
so a single-connection address is byte-identical to the four-component form. This crate is the other
half: the `SecretStore` trait that turns an address into a value, a `MemoryStore` for tests, a
portable durable `FileStore`, and an optional Vault KV v2 store.

```rust
use connector_secrets::{CredentialRef, MemoryStore, Secret, SecretStore};

let store = MemoryStore::new();
let reference = CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token")?;
store.put(&reference, &Secret::new("…")).await?;
let secret = store.get(&reference).await?;
```

Hosts admitting a second connection can enumerate one validated tenant/authority scope and migrate
the old sole-connection addresses as one checked batch. Inventory contains addresses only. Memory
and file stores provide the atomic guarantee; backends that cannot, including the current Vault
adapter, return `StoreError::Unsupported` rather than exposing a partial migration.

```rust
use connector_secrets::{CredentialScope, SecretBatch};

let scope = CredentialScope::new("9f3a4b2c", "com.zendesk.api")?;
let addresses = store.references(&scope).await?;
let mut migration = SecretBatch::new(scope);
migration.move_secret(addresses[0].clone(), instanced_reference)?;
store.apply(&migration).await?;
```

## Portable durable storage

`FileStore` is the same public `SecretStore` backend on Linux, macOS and Windows. It keeps one
logical v1 format and one contract for bounded reads and writes, atomic whole-file replacement and
all-or-nothing `SecretBatch` mutations. A host does not inspect a batch's private mutations or handle
credential values differently to select the native backend.

The platform protection is deliberately specific:

- On Unix, a new state directory is `0700` and a new credential file is `0600`. The current process
  must own existing objects. A foreign owner, wider mode, symlink, wrong object kind or metadata that
  cannot be inspected is refused before a value is read or written; it is never repaired silently.
- On Windows, new objects are owned by the process token's `TokenUser` SID and use a non-null,
  protected DACL granting access only to that SID. A foreign owner, inherited allow entry, allow
  entry for another SID, reparse point, wrong object kind or unreadable security descriptor is
  likewise refused without repair.

This is owner-only persistence, not encryption. Anyone able to bypass those controls can recover the
logical file bytes; Unix root, Windows administrators and copied backups are outside the guarantee.
`FileStore` is for one local operator and one writer. Use a real secret service such as Vault for a
shared or multi-operator deployment.

Put the file in a conventional per-user state directory or an owner-only child directory. A path
directly beneath a shared directory such as `/tmp` is refused; do not narrow the shared ancestor's
permissions to make a credential store pass its checks.

`Secret` does not implement `Serialize`, `Display` or `Debug` over its value — that is asserted by a
`compile_fail` doctest rather than merely documented.

## What it is not

Part of the compiler. The crate is fenced out of the flux-connectors compile path by a test over the
resolved dependency graph — including optional dependencies, so adding the edge behind a feature
flag trips it too. `vault` (and with it `reqwest`) is off by default: a consumer that wants only the
trait, addressing, `MemoryStore` and `FileStore` links no HTTP client at all.

## License

MIT OR Apache-2.0.
