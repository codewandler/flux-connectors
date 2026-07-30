---
id: C-47
title: Design a db.open seam so connectors can reach databases
pillar: Bridge
status: ready
priority: 8
design: docs/designs/auth-seam.md
epic: unified-auth
areas: [flux-bridge, connector-flux]
note: the $auth argument applied to a second transport · unblocks mysql-class connectors
---

# Design a db.open seam so connectors can reach databases

## Goal
Give generated Flux a way to reach a database at all: a `db.open` op that abstracts the engine
behind one interface and resolves credentials host-side, so a database connector becomes generated
rather than hand-written.

## Acceptance
- [ ] A design record specifying `db.open`: what it takes, what it returns, and how a later query op
      uses that handle.
- [ ] **The engine is abstracted.** A connector declares "postgres" or "mysql"; the generated Flux
      does not branch on it.
- [ ] **Credentials are resolved host-side and never appear in generated Flux** — the same invariant
      the `$auth` marker establishes for HTTP. The module names a credential; the host resolves the
      DSN.
- [ ] The endpoint (host, port, database) is **operator configuration**, not baked into the
      connector — matching how `EndpointSpec` already works for plugins.
- [ ] Connection lifecycle is specified, including what closes a handle on an error path.
- [ ] Paste-ready flux story drafts, in the style of
      [auth-seam-flux-stories.md](../designs/auth-seam-flux-stories.md), each naming its
      failing-first test.
- [ ] The consequence for [C-46](C-46-generic-connectors.md) is written back: with this seam, a
      `mysql` connector becomes possible.

## Progress
- (not started)

## Notes

### This is the `$auth` argument, applied again

The seam that unblocks HTTP connectors says: the generated module names a *credential*, and the host
resolves it, applies the scheme, and registers it with the redactor. `db.open` is the same shape for
a different transport — name a database, let the host resolve the DSN. Everything the
[auth-seam](../designs/auth-seam.md) establishes about why assembly must not happen in generated
Flux applies unchanged: a DSN embeds a password, so building one in a `fmt(…)` would put a
credential in a model-visible symbol and defeat redaction.

### It corrects C-46's mysql verdict

C-46 records that mysql "does not fit" because everything this repo emits goes through
`http.request`. That reasoning was **too strong**: the emitter is bound to whatever operations flux
registers, not to HTTP intrinsically. With `db.open` registered, a generated `.flux` reaches a
database the same way it reaches an API. The correct statement is *"not reachable with today's op
catalogue"*, and this story is what changes the catalogue.

### The prior art is already in flux, and should be reused

flux's **`sql` plugin** (`../flux/plugins/sql/src/main.rs`) already declares an `sql.endpoint`
`EndpointSpec` and a `dsn` `AuthMethod`, exposes `sql.query_rows`, and carries engine-specific
handling including MySQL. So the abstraction exists — as a **plugin**, reached over stdio, rather
than as a first-class op a generated module can call.

The open question is therefore not "how do we abstract a database" but **"should `db.open` be a new
builtin, or should connectors be able to call plugin operations?"** The second is a much larger
change to what a connector is, and it deserves stating rather than being assumed away. There is also
a narrower builtin already present — `sqlite_query` (`db, sql[, params]`, read-only) — which is
evidence flux is willing to have database ops in the core catalogue.

### Lifecycle has a natural primitive already

Flux-Lang has a **`scope`** node: RAII-style acquire → use → release with guaranteed cleanup, where
`finally` runs on normal completion, an early `return`, *or* an error. That is exactly the shape an
open/close pair needs, and it means the emitter would not have to invent connection lifecycle — it
emits a `scope` whose `acquire` is `db.open`.

### Boundary check

A database connector still sits on the "technology, not SaaS" side of the charter that `AGENTS.md`
draws, and flux already ships the `sql` plugin. So this story does **not** by itself argue that
databases belong here — it removes the *technical* impossibility, and leaves the *charter* question
to C-46. Both need answering before a `mysql` provider is written.
