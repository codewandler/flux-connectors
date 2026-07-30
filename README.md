# flux-connectors

Compiles vendor API specs into **Flux-Lang**.

Integrating a SaaS product into [flux](https://github.com/codewandler/flux) normally means writing a
stdio plugin — a large hand-written Rust artifact for a handful of operations. But almost everything
such a plugin encodes (base URL, auth kind, endpoints, parameters, response shapes) is already
published by the vendor. A **connector** is what remains once you stop hand-writing the part a
machine can derive: **auth + operations + quirks**.

Describe a provider once in `providers/<name>.toml`. The build emits committed, reviewable artifacts:

- `connectors/<name>.flux` — typed `op` declarations, built as real `flux_lang` AST nodes and
  formatted by flux-lang's own formatter, **never** by string templates.
- `connectors/<name>.connector.toml` — the capability manifest.

flux loads the module from `~/.flux/flows` and every `op` becomes a first-class operation and an LLM
tool. **No credential ever enters a provider TOML, a generated `.flux` file, or the lockfile**: the
generated call carries an auth *reference* the host resolves.

## Status — v0.0.1

Early. The pipeline works end to end and three providers compile; **nothing here can make a live API
call yet.** See [Known limits](#known-limits).

```bash
cargo run -p connector-cli -- build
# 3 providers, 6 artifacts; 6 written
```

Shipping today: **zendesk** (7 operations), **freshdesk** (9), **babelforce** (9) — 25 operations
curated from 186 available.

Every operation is also embedded in the `connector-catalog` crate, so a Rust consumer gets the whole
catalogue with `cargo add` rather than by copying files:

```rust
let op = catalog::operation(catalog::OperationKey::id("zendesk-ticket-show")).unwrap();
op.flux;          // the generated Flux source
op.risk;          // Risk::Low
op.credentials;   // OR of AND-mechanisms
```

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/readme-snippet-dark.svg">
  <img alt="Generated Flux for zendesk-ticket-comment-add, syntax highlighted" src="assets/readme-snippet-light.svg">
</picture>

<details>
<summary>Same thing as raw text (copy-pasteable)</summary>

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

**TOML is input to a compiler; the artifact that runs is Flux.** Flux is a real typed language with a
parser, an analyzer, a formatter, and first-class `retry`, `throttle`, `saga` and approval gates — so
there is no second little language, and no template DSL to grow one.

**The vendor spec is the source of truth; drift is detected, not absorbed.** Every artifact records
the hashes that produced it, and `flux-connectors check` fails when upstream moves.

**Generated code is committed and reviewed** — an explicit CLI run producing a diff a human reads,
never build-script magic and never a network call at run time.

## Layout

| Crate | Role |
|---|---|
| `crates/connector-spec` | The connector IR and its front-ends (provider TOML, OpenAPI). No network IO. |
| `crates/connector-flux` | Emits Flux-Lang modules from the IR via `flux_lang`'s AST and formatter. |
| `crates/connector-cli` | The `flux-connectors` binary: `build`, `diff`, `check`, `fetch`, `install`. |
| `crates/catalog` | `connector-catalog` — every operation's Flux embedded at compile time, queryable without touching the filesystem. |

## Build

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The documentation site is a separate, self-contained Node build under [`web/`](web/README.md) — it
does not participate in the cargo workspace:

```bash
cd web && npm ci && npm run build   # requires Node 22+
```

## Known limits

Stated plainly, because a connector that looks like it works and doesn't is worse than one that says
it doesn't:

- **No provider can make a live call yet.** All three need credentials, and flux's `http.request`
  cannot express any of their auth schemes — its `{"$secret": "ENV"}` marker is a whole-value
  replacement, so it produces neither a `Bearer ` prefix nor a base64-joined Basic pair. The fix is
  designed in [docs/designs/auth-seam.md](docs/designs/auth-seam.md) and must land in flux.
- **Freshdesk ships with no credential at all**, deliberately. Its `base64(<api_key>:X)` puts the
  secret in the *username* position, which the IR cannot yet mark as secret — so it would escape
  secret gating and redaction. Fail-closed 401s beat a leaked key.
- **`zendesk-ticket-search` is non-functional.** Query values are not percent-encoded and flux has no
  op that does it. Note that `url::Url::parse` already rescues *spaces*, so a casual test looks fine
  while `&`, `#` and `+` corrupt the request — and a value like `x&per_page=1` injects parameters.
  See [docs/designs/query-encoding.md](docs/designs/query-encoding.md).
- **Base URLs carry unbound template variables** (`https://{subdomain}.zendesk.com`) with no env
  binding yet.
- **OpenAPI ingest is not wired.** All three providers are hand-authored; the loader refuses a
  `[spec]`-backed provider rather than emitting an empty module.

## Docs

| If you want | Read |
|---|---|
| Why this exists, and the principles | [docs/vision.md](docs/vision.md) |
| What ships next, and the epics | [docs/roadmap.md](docs/roadmap.md) |
| **The operating contract, if you are an agent** | [AGENTS.md](AGENTS.md) |
| How a provider becomes a `.flux` module | [docs/designs/connector-pipeline.md](docs/designs/connector-pipeline.md) |
| One credential model for every provider | [docs/designs/unified-auth.md](docs/designs/unified-auth.md) |
| The status board | [docs/stories/README.md](docs/stories/README.md) |
| How the docs site is built and deployed | [web/README.md](web/README.md) |

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
