---
id: C-46
title: Generic connectors — http, a2a, mcp and friends
pillar: Spec
status: backlog
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec, providers]
note: extends the charter — a provider need not be a vendor · **not** mysql, see Notes
---

# Generic connectors — http, a2a, mcp and friends

## Goal
Let a provider describe a **protocol** rather than a vendor, so one connector serves any endpoint
speaking it — a generic `http` connector, an `a2a` connector for remote agents, an `mcp` connector
for tool servers.

## Acceptance
- [ ] The IR expresses a connector whose **endpoint is caller- or operator-supplied** rather than
      baked into `base_url`. Today `base_url` is a fixed string with unbound template variables and
      no declared binding — this is the same gap C-17 found, surfacing again.
- [ ] `providers/http.toml` — a generic request operation: caller supplies URL, method, headers and
      body. The thin case, and the one that proves the endpoint model.
- [ ] `providers/a2a.toml` — JSON-RPC 2.0 over HTTP against a remote agent: send a message, poll a
      task, fetch the agent card.
- [ ] `providers/mcp.toml` — **HTTP/SSE transport only** (see Notes). Tool list and tool call.
- [ ] Each still passes the C-11 parse-and-analyze gate and emits through `flux_lang`, exactly like a
      vendor connector. A generic connector is not a special case in the pipeline.
- [ ] `AGENTS.md`'s charter boundary is **updated or explicitly reaffirmed** — see the boundary
      question below. This story must not quietly redefine what belongs here.

## Progress
- (not started)

## Notes

### The charter question, which this story cannot dodge

`AGENTS.md` currently says: *"Connectors are paid SaaS services"* and *"technology adapters stay in
flux as plugins"*. A generic `http` or `mcp` connector is neither a paid SaaS service nor a
technology adapter — it is a **protocol** connector, a third category the boundary does not name.

That is a reasonable extension rather than a contradiction: the pipeline's actual requirement is
*"describable as auth + operations + quirks over HTTP"*, and a protocol satisfies it as well as a
vendor does. But it must be written down, or the boundary stops deciding anything.

### Which of the proposed four actually fit

| Candidate | Verdict | Why |
|---|---|---|
| **http** | ✅ fits | The thinnest possible connector; the endpoint *is* the parameter. |
| **a2a** | ✅ fits | JSON-RPC 2.0 over HTTP (`../flux/crates/flux-a2a/src/lib.rs:2`), so it is ordinary HTTP with a structured body. |
| **mcp** | ⚠️ partly | Expressible over its **HTTP/SSE** transport. Its **stdio** transport is not — there is no process spawning in generated Flux, by design. Ship the HTTP half, say the stdio half is out of scope. |
| **mysql** | ❌ does not fit | A **binary wire protocol**, not HTTP. Everything this repo emits goes through `http.request`; there is no primitive a generated `.flux` could use to speak MySQL. It is also exactly what flux's existing **`sql` plugin** is for (`../flux/plugins/sql/`). |

**mysql is the useful negative result.** It is not a matter of effort — the emitter targets
`http.request` and a database speaks a different wire protocol entirely. A connector cannot reach it
at all. That is precisely the line the charter boundary was drawn along, and it holds: databases are
technology adapters and flux already ships one.

### What this needs first

- **A declared endpoint binding.** C-17 found that `base_url` carries `{subdomain}` / `{domain}`
  placeholders with nothing saying where they resolve from, and C-29 left it open. A generic
  connector makes that gap load-bearing rather than latent: the whole point is that the endpoint
  comes from outside.
- **Streaming**, for the a2a and mcp cases that use SSE. `http.request` returns a byte-capped body,
  not a stream. Worth confirming how much of each protocol is reachable without it before promising
  either connector works end to end.
- The credential model already generalises — a generic connector's auth is whatever the operator
  configures, which is what the [unified-auth](../designs/unified-auth.md) axes were built for.
