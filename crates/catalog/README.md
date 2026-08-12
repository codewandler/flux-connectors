# connector-catalog

**Every generated connector operation, embedded at compile time and queryable by key.**

Part of [flux-connectors](https://github.com/codewandler/flux-connectors), which compiles SaaS API
descriptions into [Flux-Lang](https://github.com/codewandler/flux).

```toml
[dependencies]
codewandler-connector-catalog = "0.24"
```

The library is `catalog`, so you write `catalog::operation(…)`.

```rust
use catalog::{OperationKey, ProviderKey, Risk};

let show = catalog::operation(OperationKey::id("zendesk-ticket-show")).unwrap();
assert_eq!(show.risk, Risk::Low);
assert!(show.flux.starts_with("op zendesk-ticket-show("));

// "every operation in this provider" is one call.
let zendesk = catalog::operations_of(ProviderKey::id("zendesk"));
assert!(zendesk.iter().all(|operation| operation.provider == "zendesk"));
```

## What it is

Adding the crate *is* getting the catalogue. Every operation's Flux source and the metadata a caller
needs to decide whether to run it are `&'static` data baked into the binary by `include_str!` and a
generated table: no filesystem lookup, no parsing, no initialization.

**It resolves nothing but catalogue data, deliberately.** Its one dependency is
[`codewandler-connector-catalog-reader`](https://crates.io/crates/codewandler-connector-catalog-reader)
— the embedded **catalog pack**, every connector's canonical JSON document in one versioned,
digest-checked file, itself dependency-free. It is re-exported as `catalog::reader`, so the
document form of an operation is as close as its Flux form; hosts loading a newer pack from a path
use `catalog::reader::Pack::load`.

## What it is not

A runtime. This crate hands out **text** — it executes nothing and opens no socket. Flux runs the
module it loads; see
[`connector-pack`](https://crates.io/crates/codewandler-connector-pack) for the tools that dispatch
these operations inside a flux host.

Nothing here is hand-written: `flux-connectors build` generates the tables and the per-operation
Flux, and they are committed and reviewed like every other artifact in the repository.

## License

MIT OR Apache-2.0.
