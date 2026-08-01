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

It **derives** addresses — `Connector::gid_of`, `Connector::credential_ref_for` — but no longer owns
their spelling. `Pid`/`Gid`/`Oip` and `CredentialRef` live in
[`connector-address`](https://crates.io/crates/codewandler-connector-address) and are re-exported
here unchanged, so `connector_spec::address` and `connector_spec::credential` resolve as they always
did. They moved because
[`connector-secrets`](https://crates.io/crates/codewandler-connector-secrets) re-exports the
addressing into a published API, and while it lived here that put this whole crate — a compiler — on
crates.io to deliver a few hundred lines of vocabulary.

## What it is not

In the publish closure. This crate is this repository's compiler, not something a consumer adds; the
crates.io closure is `connector-address`, `connector-catalog`, `connector-secrets` and
`connector-pack`, derived from the manifests. Versions 0.7.0 and 0.8.0 went out before the
vocabulary was extracted and cannot be withdrawn; nothing new is published from here.

A client, a fetcher, or a runtime. **This crate performs no network IO.** Ingest takes bytes, so
every stage is a pure, unit-testable function; fetching lives in the `flux-connectors` binary alone,
and that fence is asserted by a test over the resolved dependency graph.

## License

MIT OR Apache-2.0.
