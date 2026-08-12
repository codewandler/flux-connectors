# codewandler-connector-resolve

Derives a connector's HTTP **request plan** from the flux-connectors catalog document — no engine,
no transport, no socket.

Each operation in the catalogue publishes a closed, total request template: a method, a URL, headers,
a structured query and a body, with `{slot}` interpolation over the tenant's connection settings and
`{"$param": …}` splices over the caller's arguments. This crate evaluates one, checks every
configuration value against the position it lands on, places the credentials it is handed, and hands
back the result as data.

```toml
[dependencies]
codewandler-connector-resolve = "0.26"
```

The library name is `connector_resolve`:

```rust
use std::collections::BTreeMap;

let zendesk = connector_resolve::document::provider("zendesk").expect("a shipped connector");
let operation = zendesk.operation("zendesk-ticket-show").expect("a shipped operation");
let base = zendesk.base_url(&operation.service).expect("its service");

let plan = connector_resolve::resolve(
    operation,
    base,
    &serde_json::json!({ "ticket_id": 35436 }),
    &BTreeMap::from([("subdomain".to_string(), "acme".to_string())]),
    &[], // assembled credentials, from whatever secret store you bound
)?;

assert_eq!(plan.request.method, "GET");
assert_eq!(plan.request.url, "https://acme.zendesk.com/api/v2/tickets/35436");
# Ok::<(), connector_resolve::Error>(())
```

A `RequestPlan` carries the request, the **permission subjects** a host's network policy should judge
it by — the URL before any credential was placed — and the set of strings a redactor must be holding
before any of it reaches a surface. Secret-bearing fields are `SensitiveText`, whose `Debug` prints
`<redacted>`.

**Projecting a plan into a `Tool` is not composing a request.** Wrap and dispatch the plan you were
handed; a consumer that edits one has become a second request path.

Every refusal refuses and none repairs: a missing parameter, a configuration value that would move
the origin or escape its path segment, a caller value that leaves a `{placeholder}` in the finished
URL, a credential whose header the template already sets, an inbound signing secret. A partly
evaluated request is not a degraded request — it is a different call, and the vendor answers it.

For the flux `Tool` pack built on top of this — registry installation, credential resolution,
approval gating and dispatch — use
[`codewandler-connector-pack`](https://crates.io/crates/codewandler-connector-pack).

License: MIT OR Apache-2.0.
