<p align="center">
  <img src="assets/brand/banner.svg" alt="flux-connectors — vendor API specs, compiled into Flux-Lang" width="820">
</p>

# flux-connectors

Compile SaaS API descriptions into typed [Flux-Lang](https://github.com/codewandler/flux)
operations, capability manifests, and a queryable Rust catalogue.

> [!WARNING]
> **v0.3.0 is a compiler and catalogue preview, not a live connector runtime.** The build works end
> to end, but none of the generated providers can authenticate and make a live API call yet. See
> [Current limitations](#current-limitations).

The repository currently contains 97 curated connector operations across 17 providers — **Fly.io**
(9), **Zendesk** (7),
**Freshdesk** (9), **babelforce** (9), **Google Workspace** (8, across three services), **Jira** (6),
**GitHub** (5), **HubSpot** (5), **Intercom** (5), **Shopify** (5), **Asana** (5), **OpenAI** (4),
**OpenRouter** (4), **Slack** (4), **Airtable** (4), **Sentry** (4) and **Zoom** (4). It also publishes
77 Flux-owned core operations, nodes, and capability records. A full build compiles everything into
236 committed, reviewable artifacts without contacting a vendor.

## Why this exists

A SaaS integration usually repeats information the vendor has already published: a base URL, an
authentication scheme, endpoints, parameters, and response shapes. flux-connectors makes that
information compiler input. The small provider definition records the curated surface and its
quirks; the compiler produces the Flux that actually runs.

This project is for generated HTTP connectors to services such as Zendesk, Salesforce, or OpenAI.
Stateful or protocol-rich technology adapters—Docker, Kubernetes, SQL, Prometheus, and similar—stay
as hand-written plugins in flux.

## What the compiler produces

Describe a provider once in `providers/<name>.toml`. `flux-connectors build` writes:

| Output | Purpose |
|---|---|
| `connectors/<name>.flux` | The provider's typed Flux `op` declarations. |
| `connectors/<name>.connector.toml` | The host-facing capability and credential manifest. |
| `crates/catalog/ops/<name>/*.flux` | One standalone rendering per operation. |
| `crates/catalog/src/generated/<name>.rs` | Static metadata embedded by `connector-catalog`. |
| `web/public/catalog.json` | The generated data behind the public operation explorer. |
| `web/public/v1/**/*.json` | Dereferenceable Flux core entries and JSON Schemas. |

Generated Flux is built as real `flux_lang` AST nodes and formatted by flux-lang's formatter—never
assembled with string templates. Generated artifacts are committed so changes arrive as ordinary,
reviewable diffs.

## Try it locally

You need Rust 1.87 or newer. From the repository root:

```bash
# Confirm the committed artifacts match their inputs; this does not write files.
cargo run -p connector-cli -- diff

# Regenerate all artifacts. This is hermetic and offline.
cargo run -p connector-cli -- build
```

On a clean checkout, `diff` reports:

```text
236 artifacts up to date (17 providers checked)
```

Then inspect [`connectors/zendesk.flux`](connectors/zendesk.flux), browse the
[catalogue explorer](https://flux.codewandler.org/explorer), or query the embedded
catalogue from Rust:

```bash
cargo add connector-catalog
```

```rust
use catalog::{OperationKey, Risk};

let op = catalog::operation(OperationKey::id("zendesk-ticket-show")).unwrap();
assert_eq!(op.risk, Risk::Low);
println!("{}", op.flux);
```

The crate contains static data only: no filesystem access, initialization, runtime, or transitive
dependencies.

### CLI status

| Command | Status |
|---|---|
| `build` | Implemented; regenerates committed artifacts offline. |
| `diff` | Implemented; reports what `build` would change without writing. |
| `check` | Planned in C-14; currently exits with an error. |
| `fetch` | Planned in C-14; currently exits with an error. |
| `install` | Planned in C-15; currently exits with an error. |

## An example of generated Flux

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/readme-snippet-dark.svg">
  <img alt="Generated Flux for zendesk-ticket-comment-add, syntax highlighted" src="assets/readme-snippet-light.svg">
</picture>

<details>
<summary>Same operation as copy-pasteable text</summary>

```flux
op zendesk-ticket-comment-add(ticket_id: Number, updated_stamp: String, body: String, public: Bool) -> Any
  description "Add a comment to a ticket; the comment is an internal note unless public is explicitly true"
  risk "medium"
  idempotency "conditional"
  effects ["network"]
  expose true

  $base = "https://{subdomain}.zendesk.com"
  $url = fmt("{base}/api/v2/tickets/{ticket_id}.json")
  $content_type = "application/json"
  $safe_update = true
  $payload = { ticket: { comment: { body: $body, public: $public }, safe_update: $safe_update, updated_stamp: $updated_stamp } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "PUT", url: $url })
  return $response
```

</details>

## Design in one screen

**TOML is compiler input; Flux is the execution format.** Flux already has a parser, analyzer,
formatter, control flow, retry and throttle primitives, sagas, and approval gates. Connector
behavior belongs there instead of in a second configuration language.

**The vendor spec is the source of truth; drift is detected, not silently absorbed.** Artifacts
record the hashes that produced them. Once C-14 lands, `flux-connectors check` will use that
provenance to detect upstream movement.

**Secrets never enter compiler inputs or generated source.** Provider TOML, generated `.flux`, and
the lockfile carry credential references only. The host is responsible for resolving credentials,
applying their scheme, and registering secret values with its redactor.

## Current limitations

These are stated plainly because a connector that merely looks executable is worse than one that
fails closed:

- **No provider can make a live call yet.** All three need credentials, and flux's `http.request`
  cannot express their auth schemes. Its `{"$secret": "ENV"}` marker replaces a whole value, so it
  produces neither a `Bearer ` prefix nor a base64-joined Basic pair. The required `$auth` seam is
  designed in [docs/designs/auth-seam.md](docs/designs/auth-seam.md) and must land in flux.
- **Freshdesk ships with no credential at all**, deliberately. Its `base64(<api_key>:X)` places the
  secret in a position the current IR cannot mark as secret. Emitting it would bypass secret gating
  and redaction, so the connector fails closed with a 401 instead.
- **`zendesk-ticket-search` is non-functional.** Query values are not percent-encoded and flux has
  no operation that can encode them. Spaces can appear to work while `&`, `#`, and `+` corrupt the
  request; `x&per_page=1` can inject a parameter. See
  [docs/designs/query-encoding.md](docs/designs/query-encoding.md).
- **Base URLs can contain unbound template variables**, such as
  `https://{subdomain}.zendesk.com`; environment binding has not landed.
- **OpenAPI ingest is not wired.** All 17 providers are hand-authored. A `[spec]`-backed provider
  is rejected rather than compiled into a plausible but empty module.
- **`check`, `fetch`, and `install` are not implemented.** Their CLI entries fail explicitly and
  point to their owning stories.

## Contributing

The workspace requires:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The documentation site is a separate Node 22+ build and does not participate in the Cargo
workspace:

```bash
cd web
npm ci
npm run build
npm test
```

Agents and automation must read the repository's [operating contract](AGENTS.md) before changing
anything. Human contributors can use the same [status board](docs/stories/README.md) and story
workflow.

## Repository map

| Path | Role |
|---|---|
| `crates/connector-spec` | Connector IR, provider loading, validation, and lockfile. No network IO. |
| `crates/connector-flux` | Flux emission through `flux_lang`'s AST and formatter. |
| `crates/connector-cli` | Filesystem and future network orchestration for the `flux-connectors` binary. |
| `crates/catalog` | Dependency-free `connector-catalog`, with every operation embedded at compile time. |
| `providers/` | Hand-authored provider definitions. |
| `specs/` | Vendored vendor-spec cache used by offline builds. |
| `docs/` | Vision, roadmap, designs, and the story board. |
| `web/` | The public VitePress documentation and operation explorer. |

More detail:

| If you want… | Read… |
|---|---|
| Why the project exists | [docs/vision.md](docs/vision.md) |
| What ships next | [docs/roadmap.md](docs/roadmap.md) |
| How a provider becomes Flux | [docs/designs/connector-pipeline.md](docs/designs/connector-pipeline.md) |
| The credential model | [docs/designs/unified-auth.md](docs/designs/unified-auth.md) |
| Current work and story status | [docs/stories/README.md](docs/stories/README.md) |
| Site development and deployment | [web/README.md](web/README.md) |

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
