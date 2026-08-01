# connector-address

**How a connector, its services, its operations and a tenant's credentials are named.**

Part of [flux-connectors](https://github.com/codewandler/flux-connectors), which compiles SaaS API
descriptions into [Flux-Lang](https://github.com/codewandler/flux).

```toml
[dependencies]
connector-address = "0.8"
```

The library is `connector_address`.

```rust
use connector_address::{CredentialRef, Gid, Layout, TenantLayout};

let gid = Gid::parse("com.amazonaws/s3:2006-03-01")?;
assert_eq!(gid.service, "s3");

let reference = CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token")?;
assert_eq!(
    TenantLayout.render(&reference),
    "tenants/9f3a4b2c/com.zendesk.api/support/api_token"
);
```

## What it is

Four addresses sharing one grammar:

```text
pid          com.amazonaws                                   the provider
gid          com.amazonaws/s3:2006-03-01                     one service of it, versioned
oip          com.amazonaws/s3:2006-03-01#object-get          one operation
credential   tenants/<tenant>/com.zendesk.api/api_token      where a tenant's secret lives
```

Every component is validated, the reserved `default` service is elided from all four, and
`parse(render(x)) == x` holds through the elision — which is what makes an address an *identifier*
and not merely a destination. A tenant id is treated as untrusted input, because it reaches a
filesystem-like path in a secret store: there is no way to construct a reference that renders a
traversing path.

## What it is not

A compiler, a store, or a client. It holds no value, opens no socket and reads no file — an address
is a **name**. [`connector-spec`](https://crates.io/crates/codewandler-connector-spec) produces these
addresses from a connector definition;
[`connector-secrets`](https://crates.io/crates/codewandler-connector-secrets) resolves a
`CredentialRef` to a value and re-exports every name here, so a consumer never has to name two
crates to spell one address.

That split is why this crate exists at all. It was two modules of the compiler, and one re-export
was enough to put the whole compiler on crates.io behind them.

## License

MIT OR Apache-2.0.
