# connector-secrets

**A host library: where a connector's `CredentialRef` resolves to a value.**

Part of [flux-connectors](https://github.com/codewandler/flux-connectors), which compiles SaaS API
descriptions into [Flux-Lang](https://github.com/codewandler/flux).

```toml
[dependencies]
connector-secrets = "0.5"
# The Vault store and its HTTP transport are opt-in:
# connector-secrets = { version = "0.5", features = ["vault"] }
```

## What it is

[`connector-address`](https://crates.io/crates/codewandler-connector-address) owns the credential
*address* —
`tenants/<tenant>/<authority>[/@instances/<uuid>][/<service>]/<credential>`, pure and validated.
The instance names which of a tenant's connections, and is carried only when it holds more than one,
so a single-connection address is byte-identical to the four-component form. This crate is the other
half: the `SecretStore` trait that turns an address into a value, a `MemoryStore` for tests, and an
optional Vault KV v2 store.

```rust
use connector_secrets::{CredentialRef, MemoryStore, Secret, SecretStore};

let store = MemoryStore::new();
let reference = CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token")?;
store.put(&reference, &Secret::new("…")).await?;
let secret = store.get(&reference).await?;
```

`Secret` does not implement `Serialize`, `Display` or `Debug` over its value — that is asserted by a
`compile_fail` doctest rather than merely documented.

## What it is not

Part of the compiler. Everything here that is worth having opens a socket, so this crate is fenced
out of the flux-connectors compile path by a test over the resolved dependency graph — including
optional dependencies, so adding the edge behind a feature flag trips it too. `vault` (and with it
`reqwest`) is off by default: a consumer that wants only the trait, the addressing and `MemoryStore`
links no HTTP client at all.

## License

MIT OR Apache-2.0.
