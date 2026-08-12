# Integrating flux-connectors into flux

**Audience:** someone building or extending a flux **host** who wants this repository's connectors to
make real calls. **Not** a contributor guide — for that, start at [AGENTS.md](../AGENTS.md).

> Measured against the release tree at **v0.22.0** on **2026-08-12**. Counts come from
> `web/public/catalog.json`, not from prose: **55 providers, 67 services, 835 operations, 53 events,
> 5 channel bindings**. All 55 providers declare an `authority`. Re-measure before quoting: `README.md`'s
> headline counts are now checked against a full build plan by
> `crates/connector-cli/tests/readme_snippet.rs` ([C-81](stories/C-81-declared-counts-are-checked.md)),
> but nothing pins the figures in this document.

## The one thing to understand first

This repository **compiles; flux executes**. Nothing here opens a socket. Integration therefore always
means the same thing: *a host binds three ports and hands the result to flux's registry.* The three
ports are the transport (`Egress`), the secret store (`Credentials`), and the connection settings
(`Configuration`). All three are constructor arguments, never globals, never the process environment.

## Three ways in

| Path | What it gives you | Usable today |
|---|---|---|
| **A — the Tool pack** (`connector-pack`) | Every operation as a first-class flux `Tool`, dotted (`zendesk.ticket.show`), individually gated, authenticated, executed through *your* `http.request`. | **Yes**, once you supply an `http.request` implementation — see [Gap 1](#gaps). This is the primary path. |
| **B — the catalogue** (`connector-catalog` / `catalog.json`) | Read-only metadata: operations, schemas, risk, credentials, hosts, and the emitted Flux as text. No execution. | **Yes.** Its one dependency is the data-only `catalog-reader`, and there is a JSON form — and a fetchable pack — for non-Rust hosts. |
| **C — the `.flux` modules** (`connectors/*.flux`) | The human-readable contract flux would have loaded from `~/.flux/flows`. | **No, and it is no longer a destination** — the modules are unauthenticated, there is no installer, and Decision 0022 superseded both closing stories. See [Gap 5](#gaps). |

Paths A and B are complementary: B tells a host *what exists*, A makes it *run*. Path C is a
retired destination, not an unfinished one — flux-roadmap Decision 0022 (2026-08-12) makes the
compiled form of a connector a catalog artifact, and Flux never grows a connector module loader.
Do not plan around Path C.

---

## Path A — install the Tool pack

### Step 0 — get the crates

The publish closure is five crates — `connector-address`, `catalog-reader`, `connector-catalog`,
`connector-secrets`, `connector-pack`, in that derived order — published as `codewandler-connector-*`
by CI on a `vX.Y.Z` tag. `catalog-reader` is the newest and sits between the address vocabulary and
the catalogue: it carries the pack and resolves nothing.

**They are on crates.io at 0.22.0**, published 2026-08-12. Everything in this step was verified by
building a crate **outside this workspace** against the registry versions — not read off the
manifests here ([C-190](stories/C-190-publish-catalog-pack-secrets.md)).

#### The minimum

```toml
[dependencies]
codewandler-connector-catalog = "0.22"    # lib `catalog`
codewandler-connector-pack    = "0.22"    # lib `connector_pack`
codewandler-flux-runtime      = "0.54"    # the engine line — see below, it is not optional
```

Three dependencies is the whole minimum for Path A plus Path B, and it compiles: `catalog::operation`,
`connector_pack::pack`, and every credential and configuration name the ports need
(`Credentials`, `Configuration`, `MemoryConfig`, `CredentialRef`, `Secret`, `SecretStore`,
`MemoryStore`, `StoreError`, `TenantLayout`) come off those two crates. Rust 1.87 or newer.

**Name the packages, `use` the libs.** The manifest keys carry the `codewandler-` prefix and the
externs do not — `use catalog::…`, `use connector_pack::…`. That is `[lib] name` doing its job, and
it needs no `package =` alias in *your* manifest: cargo binds the extern to the dependency's lib
name unless you rename it yourself. If you write `catalog = { package = "codewandler-connector-catalog", … }`
that also works; if you rename it to anything else, you have renamed the extern.

#### The flux line is a hard constraint, not a suggestion

`connector-pack` 0.22 requires the Flux engine crates at **`^0.54`**, and
`codewandler-flux-spec` at `^1.3`. `pack()` returns
`impl FnOnce(&mut ToolRegistry) -> flux_core::Result<()>`, so those types are in your signature
whether you name the crates or not. **A `0.x` requirement is semver-incompatible across minor
versions**, so a host on any other engine line does not get a warning — it gets two engines. Pinning
`codewandler-flux-runtime = "0.52"` beside pack 0.22 resolves both and fails like this:

```text
error[E0308]: mismatched types
  |     let _ = install(&mut registry);
  |             ------- ^^^^^^^^^^^^^ expected `flux_runtime::ToolRegistry`, found `ToolRegistry`
note: there are multiple different versions of crate `flux_runtime` in the dependency graph
```

There is no feature flag and no unification that rescues it. `cargo tree -d` on a correct consumer
reports **no duplicated `codewandler-*` crate at all**; if yours names one, that is the bug.

You only need to *name* `codewandler-flux-core` (to spell `flux_core::Result`) or
`codewandler-flux-spec` (to build a `ToolSpec` yourself, e.g. for a stand-in transport). Both are on
the same lines as above. A host that gets its `Arc<dyn Tool>` from flux-web needs neither.

#### What you do not need to name

`connector-pack` re-exports the vocabulary, so one address is spelled from one crate:

| You want | Comes from |
|---|---|
| `CredentialRef`, `InstanceId`, `Layout`, `TenantLayout`, `TenantInstances`, `Secret`, `SecretStore`, `StoreError`, `MemoryStore`, `FileStore` | `connector_pack` — no extra dependency; `FileStore` is native on Linux, macOS and Windows |
| `SecretBatch`, `PreparedSecretStore`, `PreparedSecretError`, `SecretTransactionGeneration`, `SecretTransactionId`, `SecretProposalDigest`, `SecretTransactionState` | `codewandler-connector-secrets` — add it |
| `TENANTS_ROOT`, `INSTANCES_SEGMENT`, `MAX_TENANT`, `validate_tenant`, `validate_instance` | `codewandler-connector-secrets` — add it |
| `VaultStore` | `codewandler-connector-secrets` with `features = ["vault"]` — the dependency alone is not enough |
| `Pid`, `Gid`, `Oip` — the **identifier** half of the vocabulary | `codewandler-connector-address` — add it; it is *not* re-exported by either crate above |

So `codewandler-connector-address` is a dependency you add **only** for `Pid`/`Gid`/`Oip`. Its
credential half reaches you through `connector-secrets`, and the part of that a host binding the
ports actually touches reaches you through `connector-pack`. `use connector_secrets::Pid` does not
compile — `no Pid in the root` — which is the check worth knowing before you go looking for it.

#### `connector-secrets` brings no Vault client

`vault` is off by default (published feature map: `default = []`, `vault = ["dep:reqwest",
"dep:serde_json"]`). On a consumer that adds all three crates, `cargo tree -i reqwest` answers
`package ID specification 'reqwest' did not match any packages`, and the resolved graph contains no
`rustls`, `hyper`, `native-tls` or `openssl` either. A host that wants only the trait, the addressing
and `MemoryStore` links no HTTP stack.

The durable `FileStore` takes one lifetime writer/recovery lease and supports one recoverable
prepared credential batch. Before upgrading it, stop every 0.19.1 writer; that release predates the
lease, so concurrent 0.19/0.20 writers are unsupported. The first transaction use migrates clean v1
state to v2, after which a fresh 0.19.1 process refuses the unknown format.

> **`codewandler-connector-spec` is no longer in the closure**
> ([C-407](stories/C-407-extract-the-credential-address-crate.md)). The fourth crate used to be the
> compiler, published only because `connector-secrets` re-exported the credential address vocabulary
> out of it. That vocabulary is `codewandler-connector-address` now. `codewandler-connector-spec`
> 0.7.0 and 0.8.0 stay on crates.io because a published version cannot be withdrawn; it shipped
> nothing at 0.9.0 or any later release and nothing new is published from it. `connector-flux` and `connector-cli` have
> never been published. The unprefixed `connector-cli` **does** exist on crates.io and is **an
> unrelated crate** — `github.com/dickwu/tauri-connector`, a Tauri plugin CLI. Never depend on an
> unprefixed name expecting this repository; the prefix is the identity.

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

Seventeen connectors (`algolia`, `asterisk`, `confluence`, `contentful`, `docusign`, `freshdesk`,
`gitlab`, `intercom`, `jira`, `mailchimp`, `newrelic`, `okta`, `salesforce`, `shopify`, `statuspage`,
`supabase`, `zendesk`) carry a `{placeholder}` in a resolved base URL — the connector's own or one of
its services' — covering **218 of 835 operations**, re-measured 2026-08-12 over
`web/public/catalog.json`. Their tenant values reach the pack through `ConfigStore`:

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

**From Rust** — `connector-catalog` is static data with no filesystem access, no initialization and no
runtime. Its one dependency is the data-only `catalog-reader`, re-exported as `catalog::reader`:

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

**Without a toolchain, or ahead of the crate** — every `vX.Y.Z` release attaches `catalog.pack` and a
one-line `catalog.pack.sha256` beside it (C-547), so a host with no Rust and no clone fetches the whole
catalogue and verifies it before parsing:

```bash
base=https://github.com/codewandler/flux-connectors/releases/download/v0.22.0
curl -fsSLO "$base/catalog.pack" -O "$base/catalog.pack.sha256"
sha256sum -c catalog.pack.sha256
```

From Rust that same file is `catalog_reader::Pack::load("catalog.pack")`, which re-checks the embedded
digest and the schema version and refuses rather than parsing on. The container format is documented in
[designs/catalog-artifact.md](designs/catalog-artifact.md) for a host that would rather read it directly.

**Read `status` carefully.** Every shipped operation currently reports `works: false`. Most carry one
catalog-scoped issue, `credential-not-injected` — *"Flux cannot yet apply connector credentials
securely at request time"*. That is a true statement about **Path C**, and it is misleading for Path A:
`connector-pack` assembles credentials in Rust and does inject them. See [Gap 4](#gaps).

---

## Path C — the `.flux` modules

`connectors/<provider>.flux` remains the human-readable contract and, historically, the artifact flux
would have loaded from `~/.flux/flows`. It is **not** an integration path, and since flux-roadmap
Decision 0022 (2026-08-12, adopted by [C-535](stories/C-535-adopt-decision-0022.md)) it is a retired
destination rather than an unfinished one:

- The modules are **unauthenticated**. `$auth` — a module naming a credential and having flux resolve
  and place it — was [C-10](stories/C-10-auth-injection-and-manifest.md), now closed as superseded:
  Flux never grows a connector module loader.
- There is **no installer**, and none is coming. `flux-connectors install` exits with an error
  pointing at [C-15](stories/C-15-install-and-live-e2e.md), now closed as superseded.

What replaced the destination: the compiled form of a connector becomes a versioned catalog
artifact — data the resolver (today's `connector-pack` assembly path) reads, instead of Flux text it
parses back. The program is [C-534](stories/C-534-catalog-artifact-epic.md) with the schema design
in [designs/catalog-artifact.md](designs/catalog-artifact.md). The documents (C-536) and the pack with
its reader (C-537) have shipped additively; the resolver and the retirement have not, and the `.flux`
modules keep being emitted until the differential gate proves the document-derived requests
byte-identical. [designs/auth-seam.md](designs/auth-seam.md) already recorded `$auth` as a road not
taken rather than a blocker; the supersession makes that permanent.

---

## Inbound: events and channel bindings

Manifests and `catalog.json` publish a connector's inbound surface — 53 events and 5 channel bindings
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
| 1 | **No `http.request` implementation in the *publishable* dependency graph.** `Egress::new` takes a configured `Arc<dyn Tool>` and none of the five published crates supplies one — confirmed from a consumer's own resolved graph, which contains no HTTP client at all. `codewandler-flux-web` **is** in `Cargo.lock` (0.54.0), but only through `crates/connectors-api`, the `publish = false` reference host. | Path A needs a transport you bring. Trivial if your host links flux-web; otherwise you must not substitute a hand-rolled client — a demo of a substitute demonstrates the substitute. | [connectors-app.md](designs/connectors-app.md) |
| 2 | ~~**The crates are unpublished.**~~ **Closed, and proved consumable.** All five are on crates.io — first 2026-07-31 (0.7.0), now **0.22.0**, with `catalog-reader` joining at 0.22.0. A crate built outside this workspace against the registry versions compiles and runs: `catalog::operation`, `connector_pack::pack(&["zendesk"], …)`, one Flux engine line, no HTTP client. | None. Depend on `codewandler-connector-*` from the registry, and read [Step 0](#step-0--get-the-crates) for the engine line you are thereby committing to. | [C-190](stories/C-190-publish-catalog-pack-secrets.md) |
| 3 | **One declared surface reaches no artifact.** `graphs` is in the IR and validated by the loader, and the canonical document refuses it rather than dropping it. The other five left this row: C-87 published `config` (85 fields across 42 providers) and `verify` (43 providers) into the manifest and the catalogue, C-536 carried a service's `roles` and `quirks.pagination` into `catalog/<name>.catalog.json`, and `quirks.rate_limit` became representable there. | A host renders a settings page and discovers the "Test connection" operation from the artifact alone; `auth.oauth2` is the complete declaration since schema 3, so an authorize URL is buildable from the published catalogue. What is still unavailable is a declared flow graph, and pagination is document-only — `catalog.json` does not carry it. | [C-87](stories/C-87-configuration-codegen.md) done, [connector-surfaces.md](designs/connector-surfaces.md) |
| 4 | **Published `status.works` remains dominated by the catalog-scoped `credential-not-injected` issue** describing the module path. `unbound-base-url-template` reads as stale too, since C-193 closed it for the pack. | A host filtering the catalogue on `works` omits operations the pack executes correctly. Filter on issue `code`/`scope`; treat `no-credential` as real and `credential-not-injected` as Path-C-only. The retired `unencodable-query-value` token may occur only in older catalogue documents. | none filed — worth one |
| 5 | **The `.flux` module path is unauthenticated, uninstallable — and now a retired destination.** Decision 0022 makes the compiled form a catalog artifact and closes [C-10](stories/C-10-auth-injection-and-manifest.md) and [C-15](stories/C-15-install-and-live-e2e.md) as superseded; the modules keep being emitted until [C-534](stories/C-534-catalog-artifact-epic.md)'s differential gate holds. | Path C is unavailable and stays so. Plan on Path A; the coming resolver keeps Path A's surface and reads document data instead of parsed Flux. | [C-534](stories/C-534-catalog-artifact-epic.md); the closed stories remain as honest history |
| 6 | **No inbound adapter.** Events and channels are published and unconsumed. | Webhooks, Socket Mode and polling are yours to build. | [C-118](stories/C-118-connector-channel-adapter.md) `ready` |
| 7 | **Form bodies still lack percent-encoding.** C-30 closed query injection with Flux 0.54's structured query field; form pairs remain text assembled by the emitter. | Keep unconstrained form values out until the body encoder published upstream as `L-101` reaches flux-lang. | [query-encoding-flux-stories.md](designs/query-encoding-flux-stories.md) |
| 8 | **Freshdesk declares no credential**, deliberately: its API key occupies the Basic *username* position, which the model treats as non-secret config, so emitting it would bypass secret gating and redaction. | All 9 of its operations fail closed with a `401`. | [C-16](stories/C-16-design-auth-seam.md) |
| 9 | ~~**No response shaping.**~~ **Closed by the bump it was waiting for** (C-403). This repository is on flux-web **0.54.0**, and since **0.43.0** `http.request`'s canonical `ToolResult.content` is the record `{status, headers, body}` — JSON encoded, `body` **parsed** when the response is a JSON object or array — with the flat `HTTP {status}\n{headers}\n{body}` block kept as the model-facing `view`. `connector-pack` returns the result unchanged, so that record is what a host gets. | **Read the record, not the block.** A caller selects `$resp.body.data.id` directly; a host that scanned the block for a status line gets no compile error from the change and must be updated. `crates/connectors-api/tests/live_egress.rs::the_response_comes_back_as_a_record_not_a_flat_string` pins the shape. A `404` is still a *result*, not an error. **One thing this did not close:** `Graph` → composite-op lowering is still refused, and its refusal text still cites the flat string — the prerequisite has landed, the lowering has not. | [C-404](stories/C-404-enable-graph-lowering.md) for the lowering; the shape itself is closed |
| 10 | **No token refresh, no OAuth acquisition, no multi-tenancy in one pack.** Out of scope since C-90: the store hands back a value and keeping it current is the host's. One `Credentials`/`Configuration` pair serves one tenant. | Build a pack per tenant; own your refresh loop. | by design |
| 11 | **The reference host is not a product.** `crates/connectors-api` (C-200) binds the ports, runs the loop and has sent real bytes — the first went to `api.anthropic.com` on 2026-07-31, recorded in that crate's README — but it is `publish = false`, loopback-only, and holds credentials in a 0600 file. | The seams are demonstrated, not merely stubbed; what is *not* demonstrated is a deployed multi-tenant host. Read `crates/connectors-api` as an exercise of the seam, not as something to run in production. | [connectors-app.md](designs/connectors-app.md) |
| 12 | **OpenAPI ingest is partly wired.** 8 of the 55 providers are `[spec]`-backed against vendored, hash-pinned vendor documents — `asterisk`, `babelforce`, `github`, `microsoft_graph`, `openai`, `stripe`, `twilio`, `zendesk`; the other 47 are hand-authored. What is missing is the *fetch* half: `specs/` is refreshed by the scripts in `scripts/`, never by the build. | Drift from a vendor's real API is undetectable by machine — the pinned hash proves what was ingested, not that upstream still matches it. | [C-14](stories/C-14-fetch-and-drift-check.md) |

### Not gaps, though they read like ones

- **Credential addressing is complete.** All 55 providers declare an `authority`. The older
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
