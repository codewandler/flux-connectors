# connector-catalog

**Every generated connector operation, embedded at compile time and queryable by key.**

Part of [flux-connectors](https://github.com/codewandler/flux-connectors), which compiles SaaS API
descriptions into [Flux-Lang](https://github.com/codewandler/flux).

```toml
[dependencies]
connector-catalog = "0.5"
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

**It has no dependencies, deliberately.** `cargo add connector-catalog` costs you exactly one crate.

## What it is not

A runtime. This crate hands out **text** — it executes nothing and opens no socket. Flux runs the
module it loads; see [`connector-pack`](https://crates.io/crates/connector-pack) for the tools that
dispatch these operations inside a flux host.

Nothing here is hand-written: `flux-connectors build` generates the tables and the per-operation
Flux, and they are committed and reviewed like every other artifact in the repository.

## License

MIT OR Apache-2.0.
