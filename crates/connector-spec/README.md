# connector-spec

**The connector IR and its two front-ends: provider TOML and OpenAPI ingest. Performs no network
IO.**

Part of [flux-connectors](https://github.com/codewandler/flux-connectors), which compiles SaaS API
descriptions into [Flux-Lang](https://github.com/codewandler/flux).

```toml
[dependencies]
connector-spec = "0.5"
```

## What it is

A connector is described once — either as a pointer at a vendor OpenAPI document plus patches, or as
a complete hand-authored definition — and normalized into one `Connector` value. A `Connector` is
**not** just a set of operations: it declares what a vendor can do in **both directions** (the
operations a host calls and the events the vendor sends back) and what an **operator** must supply
to use it — credentials, configuration, and the one read that proves the connection works.

It also owns **credential addressing**: `CredentialRef` and the
`tenants/<tenant>/<authority>/<service>/<credential>` layout, which
[`connector-secrets`](https://crates.io/crates/connector-secrets) re-exports rather than redefining.
That re-export is why this crate is published: it is in the public API of a crate consumers add.

## What it is not

A client, a fetcher, or a runtime. **This crate performs no network IO.** Ingest takes bytes, so
every stage is a pure, unit-testable function; fetching lives in the `flux-connectors` binary alone,
and that fence is asserted by a test over the resolved dependency graph.

## License

MIT OR Apache-2.0.
