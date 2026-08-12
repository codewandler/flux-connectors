# connector-pack

**Projects catalogue operations onto flux `ToolSpec`s and installs them as a Tool pack.**

Part of [flux-connectors](https://github.com/codewandler/flux-connectors), which compiles SaaS API
descriptions into [Flux-Lang](https://github.com/codewandler/flux).

```toml
[dependencies]
codewandler-connector-pack = "0.22"
```

## What it is

The seam between this repository's connectors and a running flux host. Given flux's own configured
`http.request` tool, a secret store bound to one tenant, and that tenant's non-secret connection
settings, `pack` installs one `Tool` per operation into a `ToolRegistry`:

```rust
let http = Egress::new(configured_http_request_tool);
let credentials = Credentials::new(host_secret_store, "9f3a4b2c")?;
let configuration = Configuration::new(config_source, "9f3a4b2c")?;

let mut registry = ToolRegistry::new();
connector_pack::pack(&["zendesk"], http, credentials, configuration)(&mut registry)?;

assert!(registry.get("zendesk.ticket.show").is_some());
```

One tool per operation, rather than one tool for the whole provider, is what lets a host gate
`zendesk.ticket.show` and `zendesk.ticket.delete` differently under flux's own permission envelope.

Credentials are bound to a tenant at construction and never looked up globally, and a credential
value is registered with the host's redactor before it can reach a model-visible surface.

When a host has resolved a tenant-scoped connection label to C-406's stable UUID, it binds both
ports to that instance. The original constructors remain the sole-connection form:

```rust
let credentials = Credentials::for_instance(host_secret_store, "9f3a4b2c", instance_uuid)?;
let configuration = Configuration::for_instance(config_source, "9f3a4b2c", instance_uuid)?;
```

`ConfigStore::get_for_instance` delegates to the original lookup only when no instance was named;
existing stores therefore compile unchanged and refuse a named instance until they deliberately
support it. Projection also refuses credential and configuration ports bound to different UUIDs.

## The request itself comes from `codewandler-connector-resolve`

The method, URL, headers and body of every call are derived from the canonical catalog document's
request template by
[`codewandler-connector-resolve`](https://crates.io/crates/codewandler-connector-resolve), which
links **no** `codewandler-flux-*` crate. This crate adds what needs flux: the `ToolSpec` projection,
registry admission, and dispatch through the host's `http.request`.

If all you want is *what request would this operation make* — a settings page, a connection check, a
rehearsal — depend on that crate directly and skip the engine line entirely. Its `resolve` returns a
`RequestPlan`: the request, the permission subjects a network policy should judge it by, and the set
of strings a redactor must hold. **Projecting a plan into a `Tool` is not composing a request** —
wrap and dispatch the plan you were handed.

## What it is not

A runtime, and not a host. This crate constructs a **declaration** handed to something that already
runs flux; it opens no socket of its own — every request goes out through the `http.request` tool
the host supplied. It links `flux-runtime`/`flux-spec` deliberately (a declaration must be spelled
in the host's own vocabulary) but not `flux-sdk`: the pack must be usable from a bare
`ToolRegistry`, and the SDK is the host's choice.

## License

MIT OR Apache-2.0.
