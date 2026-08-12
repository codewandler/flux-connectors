# connector-address

**How a connector, its services, its operations and a tenant's credentials are named.**

Part of [flux-connectors](https://github.com/codewandler/flux-connectors), which compiles SaaS API
descriptions into [Flux-Lang](https://github.com/codewandler/flux).

```toml
[dependencies]
codewandler-connector-address = "0.26"
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

It also owns the one destination a *tenant* supplies: an operator-approved HTTPS origin.

```rust
use connector_address::{HttpsOrigin, OriginRefusal};

// Equivalent safe spellings are one value, so equality is a comparison of destinations.
let supplied = HttpsOrigin::parse("HTTPS://GitLab.com:443")?;
assert_eq!(supplied, HttpsOrigin::parse("https://gitlab.com")?);
assert_eq!(supplied.as_str(), "https://gitlab.com");

// The connector owns every byte after the origin.
assert_eq!(
    HttpsOrigin::parse("https://gitlab.company.example/api/v4"),
    Err(OriginRefusal::Path)
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

…and one normalized value, `HttpsOrigin`:

```text
https://gitlab.com                  the reviewed default of a self-managed connector
https://gitlab.company.example:8443 one installation of it
```

`HttpsOrigin::parse` accepts a supplied origin in any equivalent safe spelling and returns the
canonical one: lowercase scheme and DNS host, standard-library spelling for an IPv4 or bracketed IPv6
literal, decimal port, and no default `:443`. `Eq`, `Ord` and `Hash` therefore compare destinations
rather than caller text, which is what a consumer needs in order to decide whether a proposed origin
is the same as an approved one. `HttpsOrigin::parse_canonical` is the stricter door for a value that
gets *published* — a provider-authored default or example — and refuses anything that is not already
its own canonical form.

Userinfo, plain HTTP, a path (`/` included), a query, a fragment, whitespace, a `{placeholder}`, an
unbracketed IPv6 literal, a malformed or non-ASCII host and a zero or out-of-range port are all
refused, as a closed `OriginRefusal` that **names the class and never retains the supplied text** —
a configured origin is a private installation's deployment detail, and a refusal is exactly where it
would otherwise be copied into a log. The accepted value does not render itself through `Debug`
either; `as_str()` is the deliberate call.

It holds no approval state: whether a canonical origin is *allowed* is a policy question for the host
that stores connections.

## What it is not

A compiler, a store, or a client. It holds no value, opens no socket and reads no file — an address
is a **name**.
[`connector-spec`](https://github.com/codewandler/flux-connectors/tree/main/crates/connector-spec)
produces these addresses from a connector definition;
[`connector-secrets`](https://crates.io/crates/codewandler-connector-secrets) resolves a
`CredentialRef` to a value and re-exports every name here, so a consumer never has to name two
crates to spell one address.

That split is why this crate exists at all. It was two modules of the compiler, and one re-export
was enough to put the whole compiler on crates.io behind them.

## License

MIT OR Apache-2.0.
