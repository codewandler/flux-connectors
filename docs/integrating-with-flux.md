# Integrating flux-connectors into flux

**Audience:** someone building or extending a flux **host** who wants this repository's connectors to
make real calls. **Not** a contributor guide — for that, start at [AGENTS.md](../AGENTS.md).

> Measured against the working tree at **v0.8.0** on **2026-08-01**. Counts come from
> `web/public/catalog.json`, not from prose: **53 providers, 60 services, 299 operations, 8 events,
> 2 channel bindings**. All 53 providers declare an `authority`. Re-measure before quoting; the
> hand-typed figures in `README.md` have drifted and [C-81](stories/C-81-declared-counts-are-checked.md) is the fix.

## The one thing to understand first

This repository **compiles; flux executes**. Nothing here opens a socket. Integration therefore always
means the same thing: *a host binds three ports and hands the result to flux's registry.* The three
ports are the transport (`Egress`), the secret store (`Credentials`), and the connection settings
(`Configuration`). All three are constructor arguments, never globals, never the process environment.

## Three ways in

| Path | What it gives you | Usable today |
|---|---|---|
| **A — the Tool pack** (`connector-pack`) | Every operation as a first-class flux `Tool`, dotted (`zendesk.ticket.show`), individually gated, authenticated, executed through *your* `http.request`. | **Yes**, once you supply an `http.request` implementation — see [Gap 1](#gaps). This is the primary path. |
| **B — the catalogue** (`connector-catalog` / `catalog.json`) | Read-only metadata: operations, schemas, risk, credentials, hosts, and the emitted Flux as text. No execution. | **Yes.** Dependency-free, and there is a JSON form for non-Rust hosts. |
| **C — the `.flux` modules** (`connectors/*.flux`) | The human-readable contract flux loads from `~/.flux/flows`. | **No** — the modules are unauthenticated and there is no installer. See [Gap 5](#gaps). |

Paths A and B are complementary: B tells a host *what exists*, A makes it *run*. Path C is a
different execution model that is not finished; do not plan around it.

---

## Path A — install the Tool pack

### Step 0 — get the crates

The publish closure is four crates — `connector-address`, `connector-catalog`, `connector-secrets`,
`connector-pack` — published as `codewandler-connector-*` by CI on a `vX.Y.Z` tag.

**They are on crates.io.** Four crates went out on **2026-07-31**, first at **0.7.0** and then at
**0.8.0** the same day ([C-190](stories/C-190-publish-catalog-pack-secrets.md)), so a registry
dependency works and the `path`/`git` workaround this section used to require is gone. Check
crates.io for the current version rather than trusting the pin below.

> **The membership changed after 0.8.0** ([C-407](stories/C-407-extract-the-credential-address-crate.md)).
> The fourth crate used to be `codewandler-connector-spec` — the compiler — which was published only
> because `connector-secrets` re-exported the credential address vocabulary out of it. That
> vocabulary is `codewandler-connector-address` now, and the next tag publishes it in
> `connector-spec`'s place. `codewandler-connector-spec` 0.7.0 and 0.8.0 stay on crates.io, because
> a published version cannot be withdrawn; nothing new is published from it.

```toml
[dependencies]
codewandler-connector-pack    = "0.8"          # brings the catalogue and secrets with it
codewandler-flux-runtime      = "0.45"         # your host already has this
codewandler-flux-spec         = "1.3"          # its own line, not the engine's
```

> **The flux line above is what this repository is on, and it is ahead of what is published**
> (C-403). `codewandler-connector-pack` **0.8.0**, the version live on crates.io today, requires
> `codewandler-flux-runtime ^0.41` — and a `0.x` requirement is semver-incompatible across minor
> versions, so **that published pack cannot be linked by a host on flux 0.45 at all**. Two versions
> of `flux-runtime` are two distinct `Tool` traits; there is no feature flag or unification that
> rescues it. `main` is on 0.45 and the next `vX.Y.Z` tag republishes the closure against it. Until
> that tag, pair pack 0.8.0 with flux 0.41 or build the pack from this repository.

Rust 1.87 or newer. `connector-pack` re-exports the credential vocabulary
(`CredentialRef`, `Layout`, `Secret`, `SecretStore`, `MemoryStore`, `TenantLayout`) so a host binding
the ports never has to name three crates to spell one address.

### Step 1 — supply the transport

`Egress` wraps the `http.request` tool **your host has already configured** — with its egress
allow-list, its private-network grant, its audit sink.

```rust
use connector_pack::Egress;

// In a host that uses flux-web:
let http = Egress::new(Arc::new(flux_web::http::HttpRequestTool::new(&web_options)));
```

The contract a substitute must honour: params are `{ url, method, headers?, body? }` exactly, and the
result is returned to the model unchanged. A stand-in that ignores `body`, or resolves `url` against a
base of its own, is not a substitute — it is a different connector.

> **This is the load-bearing gap.** `codewandler-flux-web` is absent from this repository's
> `Cargo.lock` entirely, so nothing here can construct one. Every `connector-pack` test passes a stub
> and says so. If your host already links flux-web, this step is one line and the gap does not apply
> to you.

### Step 2 — bind a secret store

Implement `SecretStore` over whatever you already have, or use one of the two shipped:

```rust
#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn get(&self, reference: &CredentialRef) -> Result<Secret, StoreError>;
    async fn put(&self, reference: &CredentialRef, secret: &Secret) -> Result<(), StoreError>;
    async fn delete(&self, reference: &CredentialRef) -> Result<(), StoreError>;
}
```

- `MemoryStore` — process-local; the process exiting is the cleanup.
- `VaultStore` — Vault KV v2, behind the `vault` feature (off by default).

**`get` must never collapse a transport failure into "not configured".** `StoreError::NotFound` and
`StoreError::Unreachable` want opposite responses from an operator, and the pack's diagnostics depend
on the distinction.

The unit of addressing is a `CredentialRef`, never a store-specific key. `TenantLayout` renders it:

```text
tenants/<tenant>/<authority>/<credential-leaf>
tenants/9f3a4b2c/com.zendesk.api/api_token
```

A credential is declared at **provider** level, so the service segment is always the elided `default`.
The leaf is `api_token`, *not* `zendesk.api_token` — the path already carries the authority, so the
vendor prefix would be said twice. Compose a `Layout` of your own if you have an existing secret
layout worth keeping; that composition is the point of the trait.

```rust
use connector_pack::{Credentials, CredentialRef, MemoryStore, Secret, SecretStore};

let store = Arc::new(MemoryStore::new());
let address = CredentialRef::new("9f3a4b2c", "com.zendesk.api", "default", "api_token")?;
store.put(&address, &Secret::new(token)).await?;

let credentials = Credentials::new(store, "9f3a4b2c")?;
```

### Step 3 — bind the connection settings

Nine connectors (`zendesk`, `shopify`, `jira`, `freshdesk`, `salesforce`, `docusign`, `okta`,
`contentful`, `statuspage`) carry a `{placeholder}` in their resolved base URL, covering **53 of 248
operations**. Their tenant values reach the pack through `ConfigStore`:

```rust
pub trait ConfigStore: Send + Sync {
    fn get(&self, tenant: &str, provider: &str, service: &str, field: Field<'_>) -> Option<String>;
}
```

`Field` is `Endpoint(name)` — a `{var}` in a service's `base_url` — or `Username(credential)`, the
**non-secret** user half of a `basic` credential. Two hard requirements:

- **The service is part of the address, not a hint.** The key is `(tenant, provider, service, kind,
  name)`. Contentful declares `delivery_space_id` and `management_space_id`, both binding
  `endpoint.space_id`, under two services reaching two different hosts; a store that ignores `service`
  sends a management write into whichever space the delivery reads were configured with, and gets a
  `200` from a real server.
- **Answers must be stable for as long as the store is bound.** A store that reads a database per call
  can answer the permission gate with one host and the request with another. If your values can move,
  resolve them eagerly and hand over a fixed set — that is what `MemoryConfig` is for.

```rust
use connector_pack::{Configuration, MemoryConfig};

let settings = MemoryConfig::new()
    .with_endpoint("9f3a4b2c", "zendesk", "default", "subdomain", "acme");
let configuration = Configuration::new(Arc::new(settings), "9f3a4b2c")?;
```

Both `Credentials` and `Configuration` carry the tenant they answer for, and projection refuses a pair
that disagrees (`Error::TenantMismatch`) — one connector serves one tenant.

### Step 4 — install into the registry

```rust
let mut registry = flux_runtime::ToolRegistry::new();
connector_pack::pack(&["zendesk", "slack"], http, credentials, configuration)(&mut registry)?;

assert!(registry.get("zendesk.ticket.show").is_some());
assert_eq!(registry.source("zendesk.ticket.show"), Some("connector-pack:zendesk"));
```

`pack` returns `impl FnOnce(&mut ToolRegistry) -> flux_core::Result<()>` — exactly what
`flux_sdk::ClientBuilder::try_register_pack` takes, and equally callable against a bare registry.
There is deliberately no `flux-sdk` dependency here: the SDK is the host's choice.

```rust
let client = flux_sdk::Client::builder()
    .try_register_pack(connector_pack::pack(&["zendesk"], http, credentials, configuration))
    .build()?;
```

Each provider installs under its own source label (`connector-pack:<provider>`) through
`try_register_all_from`, which is **atomic**: if any of a provider's operations is invalid or collides
with something already registered, none of that provider lands and flux's duplicate diagnostic names
both contributors. Nothing repairs, nothing partially installs — a pack that installed *most* of a
provider would leave a host holding a connector that silently resolves some operations and not others.

### Step 5 — call it

Operations are dotted, because every flux tool is: `zendesk.ticket.show`, `slack.chat.post.message`.
That naming asymmetry is the whole reason the pack exists — a dotted name is not a legal Flux
declaration, so the modules emit `zendesk-ticket-show` and only a tool surface can spell what flux's
flows call.

Dispatch through flux as normal. On each call the pack resolves the credential, **registers every
value with `ctx.redactor` before the request is constructed**, verifies the registration actually took,
places the value per its declared scheme (`Bearer ` prefix, basic-auth base64, query placement),
substitutes the endpoint variables, and hands `{ method, url, headers, body }` to your `http.request`
with the **same** `ctx`.

### Step 6 — verify the seam, don't assume it

Worth asserting in your own host, because each has a specific failure that looks like success:

| Assert | Why |
|---|---|
| `permission_subjects` names the **substituted** host, not `{subdomain}.zendesk.com` | The pack calls `http.request`'s `execute` directly, bypassing `Executor::dispatch`, so the projected operation's own `permission_subjects`/`intents` are the *only* place your egress allow-list is consulted for the inner call. |
| Your `Egress` really is `http.request` | `dyn Tool` cannot enforce it, and a wrongly-wired host sends every connector's traffic elsewhere. |
| A short credential is refused, not sent | flux's `Redactor::add_secret` silently ignores values under six trimmed characters. The pack turns that into `Error::UnredactableCredential` rather than sending an unredactable value. |
| An unconfigured tenant refuses by name | `MissingConfig` / `MissingCredential` name the address, never the value. A partial substitution would be a request to a *different* host, which that host answers. |

Every error variant refuses and none repairs. Treat `MissingCredential` as terminal, not retryable —
the request was never sent.

---

## Path B — consume the catalogue only

For a host that wants to *know* what exists without linking the pack.

**From Rust** — `connector-catalog` is static data with no filesystem access, no initialization, no
runtime and no transitive dependencies:

```rust
use catalog::{OperationKey, ProviderKey, Risk};

for provider in catalog::providers() { /* id, vendor, authority, auth, operations */ }
let op = catalog::operation(OperationKey::id("zendesk-ticket-show")).unwrap();
assert_eq!(op.risk, Risk::Low);
println!("{}", op.flux);            // the emitted Flux, as text
```

Note the crate's lib name is `catalog`, so dependents write `use catalog::`.

**From anything else** — `web/public/catalog.json` carries the same data plus `input_schema`,
`body_schema`, `response_schema` and per-operation `status`, and `web/public/v1/**/*.json` publishes
flux's core entries and JSON Schemas dereferenceably.

**Read `status` carefully.** Every shipped operation currently reports `works: false`. Most carry one
catalog-scoped issue, `credential-not-injected` — *"Flux cannot yet apply connector credentials
securely at request time"*. That is a true statement about **Path C**, and it is misleading for Path A:
`connector-pack` assembles credentials in Rust and does inject them. See [Gap 4](#gaps).

---

## Path C — the `.flux` modules

`connectors/<provider>.flux` remains the human-readable contract and the artifact flux would load from
`~/.flux/flows`. It is **not** an integration path today:

- The modules are **unauthenticated**. `$auth` — a module naming a credential and having flux resolve
  and place it — is [C-10](stories/C-10-auth-injection-and-manifest.md), and it is not landed.
- There is **no installer**. `flux-connectors install` exits with an error pointing at
  [C-15](stories/C-15-install-and-live-e2e.md).

`$auth` is not obsolete and is not this repository's to land: it is what would keep a generated module
executable *as Flux* by a host that has never heard of `connector-pack`. But it is off the critical
path for Path A, and [docs/designs/auth-seam.md](designs/auth-seam.md) now records a road not taken
rather than a blocker.

---

## Inbound: events and channel bindings

Manifests and `catalog.json` publish a connector's inbound surface — 8 events and 2 channel bindings
today, each binding carrying its transport, a **total** verification block (an explicit
`verification = "none"` is published as such rather than omitted), a discriminator, a delivery id, a
payload mapping and a reply binding.

A host can read all of that. **Nothing consumes it**: there is no adapter binding a channel to flux's
ingress, no signature verification implementation, and no delivery path.
[C-118](stories/C-118-connector-channel-adapter.md) owns it. Plan inbound work as its own integration,
not as a flag on Path A.

---

## Gaps

Ordered by what blocks a host soonest. Each is a recorded decision with an owning story — read it
before "fixing" one.

| # | Gap | Effect on a host | Owner |
|---|---|---|---|
| 1 | **No `http.request` implementation in the *publishable* dependency graph.** `Egress::new` takes a configured `Arc<dyn Tool>` and none of the four published crates supplies one. `codewandler-flux-web` **is** in `Cargo.lock` (0.45.0), but only through `crates/connectors-api`, the `publish = false` reference host. | Path A needs a transport you bring. Trivial if your host links flux-web; otherwise you must not substitute a hand-rolled client — a demo of a substitute demonstrates the substitute. | [connectors-app.md](designs/connectors-app.md) |
| 2 | ~~**The crates are unpublished.**~~ **Closed.** All four are on crates.io, first published 2026-07-31 (0.7.0) and now at 0.8.0. | None. Depend on `codewandler-connector-*` from the registry. | [C-190](stories/C-190-publish-catalog-pack-secrets.md), still `ready` on the board although its outcome has shipped |
| 3 | **Six declared surfaces reach no artifact.** `config` (112 fields / 40 providers), `verify` (40 providers), service `roles`, `quirks.pagination`, `graphs`, `quirks.rate_limit` are in the IR and validated by the loader, and appear in neither the manifest nor the catalogue. | A host **cannot render a settings page**, cannot discover the "Test connection" operation, cannot page a list — for connectors that declare all of it. You must supply endpoint values (Step 3) knowing only the variable names, read off each operation's emitted Flux. `site.rs` also collapses the whole `OAuth2Spec` to `oauth2: bool`, so **no host can build an authorize URL from the published catalogue.** | [C-87](stories/C-87-configuration-codegen.md) `ready`, [connector-surfaces.md](designs/connector-surfaces.md) |
| 4 | **Published `status.works` is false for every one of the 299 operations**, on a catalog-scoped `credential-not-injected` issue describing the module path. `unbound-base-url-template` reads as stale too, since C-193 closed it for the pack. | A host filtering the catalogue on `works` installs nothing, including operations the pack executes correctly. Filter on issue `code`/`scope`; treat `unencodable-query-value` and `no-credential` as real, `credential-not-injected` as Path-C-only. | none filed — worth one |
| 5 | **The `.flux` module path is unauthenticated and uninstallable.** | Path C is unavailable. | [C-10](stories/C-10-auth-injection-and-manifest.md), [C-15](stories/C-15-install-and-live-e2e.md) |
| 6 | **No inbound adapter.** Events and channels are published and unconsumed. | Webhooks, Socket Mode and polling are yours to build. | [C-118](stories/C-118-connector-channel-adapter.md) `ready` |
| 7 | **`zendesk-ticket-search` is non-functional; `form` bodies share the gap.** Query values are not percent-encoded and flux exposes no encoder a Flux *program* can call. `&`, `#` and `+` corrupt the request; `x&per_page=1` injects a parameter. | Do not expose that operation. The body half now exists upstream as `L-101` and arrives when flux-lang publishes it; the query half is still open. | [query-encoding-flux-stories.md](designs/query-encoding-flux-stories.md) |
| 8 | **Freshdesk declares no credential**, deliberately: its API key occupies the Basic *username* position, which the model treats as non-secret config, so emitting it would bypass secret gating and redaction. | All 9 of its operations fail closed with a `401`. | [C-16](stories/C-16-design-auth-seam.md) |
| 9 | ~~**No response shaping.**~~ **Closed by the bump it was waiting for** (C-403). This repository is on flux-web **0.45.0**, and since **0.43.0** `http.request`'s canonical `ToolResult.content` is the record `{status, headers, body}` — JSON encoded, `body` **parsed** when the response is a JSON object or array — with the flat `HTTP {status}\n{headers}\n{body}` block kept as the model-facing `view`. `connector-pack` returns the result unchanged, so that record is what a host gets. | **Read the record, not the block.** A caller selects `$resp.body.data.id` directly; a host that scanned the block for a status line gets no compile error from the change and must be updated. `crates/connectors-api/tests/live_egress.rs::the_response_comes_back_as_a_record_not_a_flat_string` pins the shape. A `404` is still a *result*, not an error. **One thing this did not close:** `Graph` → composite-op lowering is still refused, and its refusal text still cites the flat string — the prerequisite has landed, the lowering has not. | [C-404](stories/C-404-enable-graph-lowering.md) for the lowering; the shape itself is closed |
| 10 | **No token refresh, no OAuth acquisition, no multi-tenancy in one pack.** Out of scope since C-90: the store hands back a value and keeping it current is the host's. One `Credentials`/`Configuration` pair serves one tenant. | Build a pack per tenant; own your refresh loop. | by design |
| 11 | **The reference host is not a product.** `crates/connectors-api` (C-200) binds the ports, runs the loop and has sent real bytes — the first went to `api.anthropic.com` on 2026-07-31, recorded in that crate's README — but it is `publish = false`, loopback-only, and holds credentials in a 0600 file. | The seams are demonstrated, not merely stubbed; what is *not* demonstrated is a deployed multi-tenant host. Read `crates/connectors-api` as an exercise of the seam, not as something to run in production. | [connectors-app.md](designs/connectors-app.md) |
| 12 | **OpenAPI ingest is not wired.** All 53 providers are hand-authored and no vendor spec is vendored. | Drift from a vendor's real API is undetectable by machine. | [C-14](stories/C-14-fetch-and-drift-check.md) |

### Not gaps, though they read like ones

- **Credential addressing is complete.** All 53 providers declare an `authority`. The older
  "only two of nineteen" note in `crates/connector-pack/src/lib.rs` has been corrected; the
  "exactly seven" note in `designs/connectors-app.md` predates C-92 and is still stale.
- **Templated base URLs resolve.** C-193 and C-197 closed this; the `Configuration` port is Step 3, and
  an unbound variable refuses by name rather than reaching the wire.
- **`$auth` is not blocking Path A.** C-114/C-115/C-116 moved auth assembly into Rust, so flux's
  whole-value `{"$secret"}` marker never has to grow prefix or encoder support.

## Where to read further

| Question | Document |
|---|---|
| How the pack projects an operation, and what it guarantees | [designs/connector-tool-pack.md](designs/connector-tool-pack.md) |
| The credential model and the three auth axes | [designs/unified-auth.md](designs/unified-auth.md), [designs/credential-addressing.md](designs/credential-addressing.md) |
| The reference host, its charter, and the two vertical slices | [designs/connectors-app.md](designs/connectors-app.md) |
| What the manifest and `catalog.json` actually carry | [designs/connector-surfaces.md](designs/connector-surfaces.md), [designs/catalog-json.md](designs/catalog-json.md) |
| How a provider becomes Flux | [designs/connector-pipeline.md](designs/connector-pipeline.md) |
