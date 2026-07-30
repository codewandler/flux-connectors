# Design: the `$auth` seam for `http.request`

**Status:** proposed · **Pillar:** Bridge · **Stories:** [C-16](../stories/C-16-design-auth-seam.md)

> This design describes a change to **`../flux`**, not to this repository. It is recorded here because
> flux-connectors is the consumer that motivates it. The implementation stories belong on flux's own
> board and must ship in a flux release before this repo's milestone 1 can go green.

## Why

A generated connector calls the vendor API through flux's `http.request` op
(`crates/flux-web/src/http.rs`). Credentials reach that op today through one mechanism: a header
value may be the marker `{"$secret": "ENV_NAME"}`, which `resolve_header_value`
(`crates/flux-web/src/http.rs:234`) replaces with the environment variable's value.

That marker is a **whole-value replacement, headers only**. It cannot prefix, concatenate, or encode.
The consequences are decisive for connectors:

| Vendor auth shape | Example | Works today? |
|---|---|---|
| Raw value in a custom header | Anthropic `x-api-key: <key>`, GitLab `PRIVATE-TOKEN: <tok>` | **yes** |
| `Authorization: Bearer <token>` | OpenAI, OpenRouter, most modern SaaS | no — needs the `Bearer ` prefix |
| `Authorization: Basic base64(user:token)` | Zendesk, Freshdesk | no — needs base64 of a joined pair |
| `?api_key=<token>` query parameter | older SaaS APIs | no — the marker is headers-only |

Flux-Lang cannot close this gap from the language side: its `expr` whitelist has no `base64`, and
string interpolation over a secret would defeat redaction by materializing the token into a symbol.

The rejected workaround was to have operators store a **pre-composed** header value
(`ZENDESK_AUTH="Basic dXNlcjp0b2s="`). It unblocks everything with zero flux changes, but it pushes
credential assembly onto the operator, stores a value that is neither the username nor the token in
a form anything else can validate, and gives the host no idea what scheme it is applying. We chose
the correct end state instead.

## Approach

### 1. A second marker: `{"$auth": {"purpose": "<name>"}}`

Extend `resolve_header_value` to recognize a second marker alongside `$secret`. Where `$secret` names
an *environment variable*, `$auth` names a **purpose** — a declared credential slot whose scheme the
host knows.

Object keys survive this fine in Flux text: `fmt_obj_key` (`crates/flux-lang/src/format.rs:479`)
emits a JSON-quoted key when it is not identifier-safe, and the parser recovers it losslessly. So
codegen can emit:

```flux
http.request({url: $url, method: "GET", headers: {Authorization: {"$auth": {purpose: "zendesk.api_token"}}}})
```

### 2. Reuse `flux_plugin_protocol::AuthScheme` — do not invent a second vocabulary

flux already models exactly these four shapes for plugins
(`crates/flux-plugin-protocol/src/lib.rs:344`):

```rust
pub enum AuthScheme {
    Bearer,                    // Authorization: Bearer <secret>
    Basic,                     // Authorization: Basic base64(<user_env>:<secret>)
    Header { name: String },   // <name>: <secret>
    Query  { name: String },   // ?<name>=<secret>
}
```

`AuthMethod` already carries `purpose`, `env`, `user_env`, `scheme`, and an optional `oauth2` block.
The connector path should resolve a purpose through the **same** types and the same injection logic
the plugin host uses. Layering permits it directly: `flux-plugin-protocol` is L0 and `flux-web` is L5
(`crates/flux-codegate/src/`), so the dependency runs downward.

The payoff is that OAuth2 connectors come almost free — `OAuth2Spec` and the `flux auth login` grants
already exist for plugins.

### 3. Where purposes come from: the connector manifest

A plugin declares its capabilities in a manifest. A connector must too, or the seam would let any
generated Flux name any purpose — widening egress and credential access far beyond what flux's safety
invariants permit (*"plugin host capabilities are deny-by-default and manifest-scoped"*).

So flux-connectors emits `<provider>.connector.toml` next to `<provider>.flux`, installed to
`~/.flux/connectors/`. It mirrors the plugin manifest's `EndpointSpec` / `AuthMethod` / `Caps`:

```toml
name = "zendesk"
version = "0.1.0"
http_hosts = ["*.zendesk.com"]

[endpoint]
name = "zendesk.endpoint"
env  = ["ZENDESK_URL"]

[[auth]]
purpose  = "zendesk.api_token"
scheme   = "basic"
user_env = ["ZENDESK_USER"]
env      = ["ZENDESK_API_TOKEN"]
```

flux loads installed connector manifests and resolves them into `WebOptions` alongside the existing
`allowed_secrets` field, giving `HttpRequestTool` a `purpose -> AuthMethod` map.

### 4. Fail closed, and redact

Both behaviors already have precedent in the `$secret` path and must be matched exactly:

- **Deny-by-default.** A purpose absent from the resolved map is refused *before any value is read*,
  with the same shape of error `allowed_secrets` produces today (`http.rs:236`, the C-76 precedent).
- **Redaction.** The resolved value is registered with `ctx.redactor.add_secret(...)` exactly as
  `http.rs:248` does, so a token never surfaces in logs, traces, or model-visible output. For
  `Basic`, register the *composed* base64 value — the redactor must match what actually travels on
  the wire.
- **Host scoping.** The request URL must be checked against the manifest's `http_hosts` before
  dispatch, so a connector cannot send its Zendesk credential to an attacker-chosen host. This is
  the single most important control in this design.

### 5. `Query` needs a second injection point

The three header schemes all resolve inside `resolve_header_value`. `Query` does not — it must append
a parameter to the URL, which happens after header assembly. This is a genuinely separate (small)
change to `HttpRequestTool::execute`, and it should be its own story on flux's board rather than
being smuggled into the header-marker work.

## Alternatives considered

- **Pre-composed header value in one env var.** Zero flux changes and unblocks every header scheme
  today. Rejected: it makes the operator assemble credentials by hand, stores a value in a form
  nothing can validate, and leaves the host blind to the scheme. Kept on file as an emergency
  fallback only.
- **A `base64` builtin in flux-lang's `expr` whitelist.** Would let generated Flux compose Basic auth
  itself. Rejected: it forces the raw token into a bound symbol, defeating redaction, and it puts
  credential assembly in model-visible code.
- **A dedicated `connector.request` op in flux.** Cleaner call sites, but it is a second HTTP egress
  path — flux's invariants explicitly forbid hand-rolling a second URL guard. Reusing `http.request`
  keeps one guarded path.
- **Reading purposes from flux's config file instead of connector manifests.** Simpler, but it
  scatters a connector's definition across two places and loses the plugin symmetry that makes the
  capability story easy to reason about.

## Risks & open questions

- **Cross-repo sequencing.** This is the critical path for milestone 1, and it lands in a different
  repository on a different release cadence. Mitigation: design and file the flux stories first, and
  keep this repo's work (spec crate, codegen, golden tests) fully unblocked in the meantime — none of
  it needs the seam until the live end-to-end run.
- **Does flux want connector manifests at all?** This introduces a new installable artifact kind to
  flux. If flux's maintainers prefer to fold it into the existing plugin manifest registry, the
  design should follow that rather than add a parallel loader. **Open — resolve with flux before
  implementing.**
- **Manifest trust.** A connector manifest grants credential access, so installing one is a trust
  decision equal to installing a plugin. `flux-connectors install` must not silently widen
  capabilities; the install path should show what a manifest declares.
- **Token refresh for OAuth2 connectors** is out of scope for the first cut; declare `oauth2` in the
  manifest schema but implement only the static schemes initially.

## Acceptance / done

- `http.request` accepts `{"$auth": {"purpose": "<name>"}}` as a header value and injects per
  `AuthScheme`, with `Bearer`, `Basic`, and `Header` covered.
- An undeclared purpose is refused before any credential value is read — proven by a failing-first
  test mirroring the existing `allowed_secrets` refusal test.
- The composed value is registered with the redactor; a test asserts a token does not appear in
  captured output.
- A request whose URL falls outside the manifest's `http_hosts` is refused.
- `Query`-scheme injection lands as a follow-up story with its own test.
- flux-connectors can generate a Zendesk op that authenticates against the live API with no
  pre-composed credential anywhere.
