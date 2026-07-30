---
id: C-133
title: "The brave connector — Brave Talk's room-token HTTP surface"
pillar: Spec
status: backlog
areas: [providers]
note: "ONLY the three HTTP calls. The XMPP MUC stream stays OUT — vision.md names protocol-rich technology adapters as a non-goal, and flux already has D-205/D-206 with feasibility proven live. Blocked on two real things; read Notes before starting"
---

# The brave connector — Brave Talk's room-token HTTP surface

## Goal

Expose the **HTTP** half of Brave Talk's room handshake as a connector: the calls that mint a guest
JWT and allocate a conference focus. Nothing else.

## Scope, stated as an exclusion first

**The XMPP MUC stream is not in this story and should not be in this repository.** `vision.md`:

> **Technology adapters.** Connectors are **paid SaaS services**. The flux plugins that wrap
> *technologies* — docker, kubernetes, sql, prometheus, loki, vault, asterisk — are stateful and
> protocol-rich, and they stay core to flux as plugins.

A long-lived bidirectional XML stream is exactly that class, sitting beside `asterisk` in the list.
flux already owns it: **`D-205`** (generic prosody/ejabberd MUC over WebSocket) and **`D-206`**
(JaaS/Brave Talk token acquisition and refresh), under `docs/designs/meeting-rooms.md`, with
**feasibility proven live on 2026-07-30** against a real room. Do not re-file or duplicate that work.

What *is* connector-shaped is three plain request/response calls:

```
OPTIONS https://talk.brave.com/api/v1/rooms/<room>   -> x-csrf-token + _gorilla_csrf cookie
PUT     https://talk.brave.com/api/v1/rooms/<room>   -> 200 {"jwt": "…"}
        x-csrf-token: <token>, body {"mauP": true}
POST    https://8x8.vc/<tenant>/conference-request/v1?room=<room>
```

## Acceptance

- [ ] `providers/brave.toml` ships a `talk` service covering those three operations, and **no
      operation that speaks XMPP**.
- [ ] The acceptable-use constraint is recorded **in the connector itself**, not only in a story —
      see Notes. A consumer reading the catalogue must meet it before they make a call.
- [ ] Generated Flux parses, analyzes and is a fixed point of flux's own formatter.
- [ ] No credential value anywhere; no realistic-looking `example` on a secret field.
- [ ] The build stays a fixed point and the full gate is green.

## Notes — two blockers and a constraint, all real

**1 · The CSRF token cannot be extracted in emitted Flux today.** The `PUT` needs the
`x-csrf-token` from the `OPTIONS` response, and `http.request` returns **one flat string**
(`HTTP {status}\n{headers}\n{body}`), so no emitted operation can select a field out of a prior
response — the constraint `crates/connector-flux/src/op.rs` records. A two-call handshake with a
value threaded between them is precisely the case that does not work.

This is the same limitation [C-127](C-127-truthful-output-typing.md) is about, and the
[Tool pack](../designs/connector-tool-pack.md) is what would fix it: a Tool is Rust and can parse the
response. **This story should not start before that lands**, or it will produce operations that
compile and cannot be composed.

**2 · The endpoint is unauthenticated, and the auth model assumes a credential.** There is no API
key here — the handshake is public, which is how the open-source `brave/brave-talk` client works.
Whether this repo's `[[auth]]` model can express "no credential" without a consumer treating it as a
mistake is an open question that should be answered before the TOML is written, not during.

**3 · Acceptable use — the reason this is `backlog` and not `ready`.** flux's own design says
plainly:

> The endpoint is public and unauthenticated, and the spike used it exactly as the open-source client
> does, against a room it was invited to. A bot joining calls *at scale* is a different posture.
> **Read Brave's ToS before this is anything but a spike** — and prefer the generic XMPP backend, or
> our own JaaS tenant, for anything beyond own-room use.

Publishing this in a **public catalogue** is a meaningfully different act from one spike against
one's own room: it invites exactly the at-scale use that warning is about. So the ToS question must
be answered *before* this ships, and the answer recorded. If the answer is "own-room use only", the
connector should say so where a user will see it, or not ship.

**The supportable alternative** the same design names: an own 8x8 JaaS tenant, with our own API key
and JWT signing. That is a paid SaaS product with a real credential — squarely what this repo is for,
and free of all three problems above. **If a Brave/JaaS connector is wanted, that is probably the one
to build.**
