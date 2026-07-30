# Design: the connector pipeline — provider TOML to Flux module

**Status:** proposed · **Pillar:** Spec · **Stories:** C-2 … C-15

## Why

Integrating a SaaS product into flux today means writing a stdio plugin. `plugins/zendesk/src/main.rs`
in `../flux` is 687 lines of hand-written Rust for roughly seven operations. Nearly everything in it
— base URL, auth kind, endpoint paths, parameter names and types, response shapes — is already
published by Zendesk as an OpenAPI document. That approach does not scale to Freshdesk, Salesforce,
Intercom, OpenAI, OpenRouter, and the long tail of services.

The obvious alternative is worse. `~/babelforce/projects/integrations/action-proxy` solved this with
YAML, and it is the cautionary tale this design is written against. Its
`dist/collections/freshdesk/freshdesk.yml` is 649 hand-maintained lines. Five specific failures:

1. **YAML was the execution format.** The runtime interpreted config directly — no compile step, no
   static validation, no analyzer.
2. **A homegrown template DSL** — `:params.req_id`, `{{context.api_host}}`, ad-hoc `qs:`/`uri:` keys.
   Untyped, unanalyzable, no editor support, invented from scratch.
3. **Every action hand-written.** Nothing was derived from the vendor spec, so the config drifted
   from the real API silently and permanently.
4. **Untyped parameters** — `type: string` for everything, including dates and ids. No schema ever
   reached the caller.
5. **Credentials in the config graph** — `user: :context.api_key` threaded secrets through the
   template layer.

This design inverts all five.

## Approach

### The shape of the thing

```
providers/<name>.toml ──┐
                        ├──► [connector-spec] ──► Connector IR ──► [connector-flux] ──► <name>.flux
specs/<name>/<ver>.json ┘         │                                                     <name>.connector.toml
   (vendored, committed)          └──► connectors.lock  (provenance + hashes)
```

**TOML is input to a compiler; the artifact that runs is Flux.** Flux is a real typed language with a
parser, an analyzer, a formatter, and first-class `retry`, `throttle`, `saga`, `timeout`, and approval
gates. That single decision answers failures 1 and 2 outright — there is no second little language,
because the target language already does everything a template DSL would have grown toward.

### A connector = a manifest + a Flux module

The symmetry that makes this safe: **a plugin is a manifest plus a binary; a connector is a manifest
plus a Flux module.** Each provider generates two artifacts:

| Artifact | Installed to | Purpose |
|---|---|---|
| `<provider>.flux` | `~/.flux/flows/` | The `op` declarations. |
| `<provider>.connector.toml` | `~/.flux/connectors/` | Capability manifest: endpoint env, auth credentials + schemes, `http_hosts` allowlist. |

> **Corrected by C-16 — the manifest premise was wrong.** This design originally claimed the
> `<provider>.connector.toml` would be *installed into flux* at `~/.flux/connectors/` and read as a
> capability grant, mirroring the plugin manifest. Verification against flux source disproved the
> premise: **flux has no file-based capability manifest of any kind.** A `PluginManifest` is obtained
> by *spawning the binary* and sending a `manifest` frame
> (`../flux/crates/flux-plugin/src/host/loading.rs:187-189`); what sits on disk in `~/.flux/plugins`
> is a `PluginDescriptor` carrying transport + sha256 only, never capabilities. A TOML file in
> `~/.flux/connectors/` would be flux's **first** capability grant with no binary-hash anchor, so
> `spawn_verified`'s drift refusal (D-48) would have no analogue. flux also has no "connector"
> concept to fold into.
>
> So the manifest **stays in this repo** as a build artifact and a record of what a connector needs;
> it is not an installable capability grant. Credentials reach flux through its **operator config**
> instead. See [auth-seam.md](auth-seam.md) for the resolved design and
> [unified-auth.md](unified-auth.md) for the credential model.

The manifest's shape still mirrors the plugin protocol's `EndpointSpec` / `AuthMethod` / `Caps`
(`../flux/crates/flux-plugin-protocol/src/lib.rs:422`), because agreeing with flux's vocabulary is
what makes the operator config mechanical to produce.

### Why generated Flux loads with no flux change

`DynamicComposites::load` (`../flux/crates/flux-flow/src/composites.rs:97`) reads every `.flux` file
in `~/.flux/flows` (the `@global_flows` named root) and `.flux/flows`, and lifts every `op`
declaration out of it (`load_flows_dir`, `composites.rs:302`). Composite ops already carry
`description`, `risk`, `idempotency`, `effects`, `limits`, `expose`, and `view`
(`../flux/crates/flux-lang/docs/syntax.md:164`) — precisely the `ToolSpec` surface an operation
needs. `expose true` surfaces the op to the model as an LLM tool.

Unresolvable composites are *pruned with an audit record*, not fatal (`prune_unresolvable`, C-117),
so a malformed connector degrades the catalog rather than bricking startup.

**Only auth requires a flux change**, and it is designed separately in [auth-seam.md](auth-seam.md).

### Two front-ends, one IR

The provider TOML must be able to **fully express** an operation, not merely patch one. Vendors with
no usable spec (Ollama, parts of Anthropic) are hand-authored in the same schema and travel the same
codegen path. Spec ingest is then just a way to *pre-fill* the IR.

```
Connector {
  id, vendor, base_url (with tenant templating),
  auth: Vec<AuthMethod>,          // credential name + scheme + env/user_env
  operations: Vec<Operation {
    id,                            // the op name, e.g. zendesk.ticket.show — a stable public contract
    method, path,
    params: { path, query, header, body },   // each carrying its JSON Schema
    response_schema,
    risk, idempotency, description,
    quirks: { pagination, rate_limit, error_envelope },
  }>,
  provenance: { source_url, fetched_at, spec_sha256, toml_sha256, ir_sha256 },
}
```

Parameter and response schemas travel intact from the vendor spec into the op contract — failure 4
answered.

### The overlay layer

The TOML overlays the extracted spec deterministically: **spec → patch → validate**. A patch may
select or hide operations (a 400-endpoint spec must not become 400 tools), rename them to a stable
op id, override `risk`/`idempotency`, correct a wrong type, or attach quirks. Merge order is fixed
and total so the same inputs always produce byte-identical output.

### Provenance and drift

`connectors.lock` records, per provider: spec URL, upstream version, spec sha256, TOML sha256,
generated-artifact sha256, and the generator version. `flux-connectors check` recomputes and exits
non-zero on any mismatch — so upstream drift and stale artifacts are *detected* rather than absorbed.
Failure 3 answered.

The spec cache under `specs/<provider>/<version>.json` is **vendored and committed**, which makes
builds hermetic, offline, and reviewable.

### Generation is an explicit, reviewed step

Not a `build.rs`, and never a network call at runtime. `flux-connectors build` writes artifacts a
human reads as a diff in a PR. This is a deliberate correction of both the action-proxy mistake and
of build-script magic.

### Emitting Flux

`connector-flux` builds real `flux_lang::ast` nodes and formats them with flux-lang's own formatter,
via a crates.io dependency on `codewandler-flux-lang` (lib `flux_lang`) pinned to a published
version. Unparseable
or non-canonically-formatted output is therefore structurally impossible. Illustrative output:

```flux
op zendesk.ticket.show(ticket_id: Number) -> Any
  description "Show one Zendesk ticket by id"
  risk "low"
  idempotency "idempotent"
  effects [network]
  expose true

  $url = fmt("{base}/api/v2/tickets/{ticket_id}.json")
  retry 3 backoff exponential delay 500 -> $res
    http.request({url: $url, method: "GET", headers: {Authorization: {"$auth": {credential: "zendesk.api_token"}}}})
  return $res.body
```

Note what the generated artifact does *not* contain: any credential. It carries a reference the host
resolves — failure 5 answered.

Quirks compile into Flux control flow: rate limits into `throttle`, transient failures into `retry`,
pagination into a bounded `repeat`/`each`, and non-idempotent multi-step writes into `saga`.
action-proxy's YAML could never express any of these.

## Alternatives considered

- **String-templated `.flux` output.** Fully decouples this repo from flux internals. Rejected: it
  can emit invalid Flux, and the failure is caught late and only if a flux binary is present. The registry
  pin costs an occasional bump and buys structural correctness.
- **A `build.rs` that fetches specs and generates at compile time.** Rejected: non-hermetic, invisible
  in review, and network-dependent builds. Committed artifacts are the point.
- **Emitting one `flow` per operation instead of one `op`.** Flows are entry points, not composable
  operations; only `op` declarations carry the `risk`/`idempotency`/`effects`/`expose` metadata and
  register into the tool catalog.
- **Generating Rust plugins from specs instead of Flux.** Keeps the existing plugin machinery, but
  reintroduces a compile-and-distribute-a-binary step per provider and forfeits `retry`/`throttle`/
  `saga` for free.
- **One crate instead of three.** Rejected: keeping all network IO in `connector-cli` is what lets
  `connector-spec` be a pure, fully unit-testable function from bytes to IR.

## Risks & open questions

- **Real specs are bad.** Vendor OpenAPI documents are frequently incomplete, wrong, or enormous.
  The overlay layer is the pressure valve, and its expressiveness is the main design risk — if
  patching a provider is harder than hand-writing it, the thesis fails. Zendesk is the first
  real test of this.
- **Op naming is a public contract.** `zendesk.ticket.show` is what users and models call. Names must
  be stable across regeneration and must not be derived from volatile spec fields like `operationId`
  without a pinned override.
- **Tool-catalog bloat.** Exposing too many ops degrades model performance. Selection must be
  opt-in per operation, not opt-out.
- **The flux-lang pin will drift.** Needs a routine bump-and-verify step, surfaced by `check`.
- **Response shaping is unsolved.** Returning whole API payloads will blow context budgets. `view`
  and response projection are likely needed sooner than expected; deliberately deferred past
  milestone 1.

## Acceptance / done

- A provider TOML plus a vendored spec compiles to a `.flux` module and a `.connector.toml` manifest.
- Every generated module **parses and analyzes** against flux-lang in CI.
- Golden-file tests pin generated output, so codegen changes surface as reviewable diffs.
- `flux-connectors check` fails on upstream spec drift or a stale artifact.
- `anthropic` and `zendesk` both install into `~/.flux/flows` and register as ops in a live flux
  session, with one live API call succeeding.
