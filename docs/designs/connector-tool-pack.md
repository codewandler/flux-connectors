# Design: the connector Tool pack — the flux interop layer

**Status:** approved · **Pillar:** Bridge · **Stories:** [C-113](../stories/C-113-tool-pack-epic.md) … C-118

> This design describes a new crate **in this repository** plus a small set of stories on **`../flux`**'s
> board. Every `path:line` below was read in `/home/timo/projects/flux` at `codewandler-flux-lang`
> **0.39.0**. Symbol names are stable and line numbers are not — re-grep by symbol rather than
> trusting a number that does not land.

## Why

This repo compiles vendor specs into Flux modules and a catalogue, and **nothing consumes them at
runtime**. `install` is unimplemented (C-15), so the only route into flux is a human copying `.flux`
files into `~/.flux/flows`.

flux is blocked on this by name. Release 0.38 **removed** `flux-plugin-zendesk` before its first
release, "to be superseded by a flux-connectors interop layer". `D-200`, `D-201` and `D-202` are
`blocked`, and `A-136`'s reference flow is retained-but-unrunnable, all waiting on that layer.
`examples/zendesk.triage.flux` is kept deliberately as *"the authored shape the replacement has to
satisfy"* — a written acceptance target. It calls `zendesk.test`, `zendesk.ticket.show`,
`zendesk.ticket.search` and `zendesk.ticket.comment.list`.

## The runtime already exists, and it is not ours

`flux_sdk::ClientBuilder` (`crates/flux-sdk/src/lib.rs:371`) already is the runtime-construction API,
with the ports and configuration bound at build time:

| concern | what flux already offers |
|---|---|
| bind ports/adapters | `approver(Arc<dyn Approver>)` · `with_authorization` · `with_redactor` · `storage` · `with_sandbox` · `try_with_live_datasource` |
| register operations | `register_pack(FnOnce(&mut ToolRegistry))` · `try_register_pack` · `register_op_from(source, tool)` · `with_plugin_tools_from` |
| configuration | `max_tokens` · `max_iterations` · `context_budget(bytes)` · `with_compaction` · `max_calls` · `allow`/`deny` |

`ToolRegistry::try_register_all_from` (`crates/flux-runtime/src/lib.rs`) installs a pack atomically
under one auditable source label: if any declaration is invalid or collides, none of the pack lands.

So **this repo builds no runtime.** It supplies a pack; flux constructs and runs it. `vision.md`'s
non-goal stands unamended — *"This repo compiles; flux executes. flux-connectors ships no server, no
daemon, and no request path of its own."*

## Why a Tool pack rather than composite `.flux` text

### The naming asymmetry is decisive

`crates/connector-flux/tests/op_emitter.rs` already asserts that **a dotted op *declaration* name
does not parse in flux-lang**, which is why this repo emits `zendesk-ticket-show`. But every flux
**tool** is dotted — `http.request` (`crates/flux-web/src/http.rs:83`), `command.invoke`,
`op.register`, `skill.load`.

flux's reference flow calls `zendesk.ticket.show`. It was written against a *tool* surface, and only
a tool surface can spell it.

### The safety argument is stronger

`ToolSpec` (`crates/flux-spec/src/lib.rs:289`) carries `name`, `description`, `input_schema`,
`output_schema`, `effects`, `risk`, `idempotency`, `access` and `group` — and this repo's IR already
holds every one of those, per operation.

As a composite, an operation inherits whatever gating `http.request` happens to get. As a Tool, each
operation is gated **individually** by flux's permission and approval envelope, at the risk level the
connector author declared. That is a capability the composite path cannot have.

## This dissolves the `$auth` blocker

[auth-seam.md](auth-seam.md) and C-26 exist because flux's `{"$secret": "ENV"}` marker is
*whole-value, headers-only, no prefix, no encode* — so `Bearer <token>` and basic-auth base64 cannot
be expressed, which blocks every provider from making a live call.

**A Tool builds its own header value in Rust.** The prefix and the base64 happen here, before
`http.request` ever sees the request, so the marker never needs to grow those capabilities. The
secret is kept off every surface with `ctx.redactor.add_secret(...)` — `pub redactor: Redactor` at
`crates/flux-runtime/src/lib.rs:1226` — which is exactly what `flux-web` does at
`crates/flux-web/src/http.rs:248`.

The seam design is not deleted; a composite-based connector still wants it. But **milestone 1 no
longer waits on a flux release**, and C-26's 11 paste-ready drafts should not be filed as written.

## The shape

```rust
let client = flux_sdk::Client::builder()
    .try_register_pack(connector_pack::pack(&["zendesk", "slack"]))
    .build()?;
```

Each Tool is thin and holds no transport of its own:

```rust
impl Tool for Operation {
    fn spec(&self) -> ToolSpec { /* projected from the catalogue entry */ }

    // MUST mirror http.request's own gate — see below.
    fn permission_subjects(&self, params: &Value) -> Vec<String> { vec![self.url(params)] }
    fn intents(&self, params: &Value) -> IntentSet { /* NetworkFetch */ }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> Result<ToolResult> {
        let request = self.build_request(ctx, params)?;   // url, method, headers, body
        self.http.execute(ctx, request).await             // flux owns egress
    }
}
```

`flux_web::http::HttpRequestTool` is public (`crates/flux-web/src/http.rs:38`), so delegation is a
plain method call passing the **same** `ctx`.

### Two safety must-dos, or delegation silently loses a gate

**1 · Mirror the network gate.** `HttpRequestTool::permission_subjects` returns the request URL
(`crates/flux-web/src/http.rs:118`) and `intents` raises `NetworkFetch` (`:126`). Calling `execute`
directly **bypasses `Executor::dispatch`**, so neither is ever consulted for the inner call. The
generated Tool must therefore declare the same subject and intent itself.

The connector manifest's `http_hosts` (C-10) is exactly the declared data for this. A test must
assert that every generated Tool's `permission_subjects` is non-empty and names the vendor host —
without it, installing a connector silently becomes a hole through the host's network policy. This is
the single most dangerous way this design can be implemented wrongly while appearing to work.

**2 · Register the secret with the redactor before the request is built**, not after, so a failure
between construction and dispatch cannot surface it in an error.

## Ports the host binds

- **`CredentialStore`** — the adapter this repo already modelled and never wired to anything.
  `crates/connector-spec/src/credential.rs` holds `CredentialRef`, the `Layout` trait and
  `TenantLayout` (C-90). Managing expiring tokens was out of scope there and remains so.
- **`HttpRequestTool`** — injected rather than constructed, so a host can supply a pre-configured one.

## Channels

`flux-channels` has a `Channel` trait (`crates/flux-channels/src/channel.rs:16`) and adapters `a2a`,
`schedule`, `slack`, `webhook`. [C-82](../stories/C-82-channel-bindings-epic.md) already recorded that
flux's dispatch is a closed match with one arm per vendor, and that **its slack arm hand-builds a
`chat.postMessage` this repo already compiles**. A connector-backed `Channel` adapter is the second
surface; it lands after operations, depends on the same credential port, and has a far smaller
consumer.

## Configuration — a correction worth stating plainly

`context_budget(bytes)`, `max_iterations`, `max_tokens`, `max_calls` and
`max_inflight_per_principal` exist in `flux-config`. **There is no max-memory knob and no general
concurrency limit** — the only concurrency control is server-side per-principal. Those are flux-side
stories, not ours, and nothing in this design depends on them.

## The risk to name now

Two surfaces are generated from one IR — the `.flux` module and the Tool pack — and **they can drift
into disagreeing about the same operation**. `AGENTS.md` already warns about exactly this for the
C-12/C-95 shared lowering.

Both must be generated from the same IR in one build and covered by the existing fixed-point gate. A
**differential test** asserting that the pack's constructed request and the module's emitted request
agree is the honest guard. It belongs in C-117, not in a later postmortem.

## Out of scope

- **A runtime, a server, or a request path here.** flux constructs and runs; this crate is a pack.
- **Refreshing expiring tokens** — out of scope since C-90 and still is.
- **Composite emission going away.** `connectors/*.flux` keeps shipping. The pack is an additional
  surface, not a replacement, and the `.flux` artifact remains the human-readable contract.
