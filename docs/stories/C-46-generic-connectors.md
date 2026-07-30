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
Let a provider describe a **higher protocol** rather than a vendor, so one connector serves any
endpoint speaking it — an `a2a` connector for remote agents or an `mcp` connector for tool servers.
Raw HTTP itself is already Flux's `http.request` core operation and is published by C-112; compiling
a second copy here would create two authorities for one primitive.

## Acceptance
- [ ] The IR expresses a connector whose **endpoint is caller- or operator-supplied** rather than
      baked into `base_url`. Today `base_url` is a fixed string with unbound template variables and
      no declared binding — this is the same gap C-17 found, surfacing again.
- [ ] Raw HTTP is explicitly **not** implemented as `providers/http.toml`; C-112 exposes the real
      `http.request` `ToolSpec` in the explorer and this story does not duplicate it.
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
| **http** | ↗ core | Already the native `http.request` operation; C-112 publishes its real schema rather than recompiling a weaker copy. |
| **a2a** | ✅ fits | JSON-RPC 2.0 over HTTP (`../flux/crates/flux-a2a/src/lib.rs:2`), so it is ordinary HTTP with a structured body. |
| **mcp** | ⚠️ partly | Expressible over its **HTTP/SSE** transport. Its **stdio** transport is not — there is no process spawning in generated Flux, by design. Ship the HTTP half, say the stdio half is out of scope. |
| **mysql** | ⛔ blocked, not impossible | A **binary wire protocol**, not HTTP — unreachable with **today's** op catalogue. See the correction below. |

**Correction — the original mysql verdict here was too strong.** It said a connector "cannot reach a
database at all". That reasoning was wrong: the emitter is bound to **whatever operations flux
registers**, not to HTTP intrinsically. A `db.open` op — abstracting the engine and resolving
credentials host-side, exactly as the `$auth` marker does for HTTP — makes a database reachable from
generated Flux. That seam is [C-47](C-47-db-open-seam.md).

So mysql is **blocked on a missing primitive**, not impossible. Two separate questions remain, and
both need answering before a `mysql` provider is written:

1. **Technical** — does `db.open` exist? (C-47.) flux already has the pieces: a `sql` plugin
   declaring an `sql.endpoint` and a `dsn` credential, and a `sqlite_query` builtin.
2. **Charter** — *should* a database live here at all, given `AGENTS.md` puts technology adapters in
   flux and flux already ships the `sql` plugin? That question is this story's, and it is unaffected
   by C-47.

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
