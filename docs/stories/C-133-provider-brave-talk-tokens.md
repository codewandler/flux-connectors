---
id: C-133
title: "The brave connector — Brave Talk's room-token HTTP surface"
pillar: Spec
status: blocked
priority: 5
areas: [providers]
note: "BLOCKED — its own Notes say it must not start before C-127 (still ready), the acceptable-use question is unanswered, and C-206 (found here) must settle how a genuinely-public endpoint is published. Charter question too: vision.md scopes connectors to paid SaaS"
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

## Progress

- **Scheduled.** flux's meeting-rooms epic moved to the front of its board (`D-203` p3, `D-205` XMPP
  p4, `D-206` Brave Talk p5), so this connector's counterpart is being built there. The XMPP stream
  stays in flux; this story remains only the three HTTP calls.

- **Two of the three blockers named above still stand**, and scheduling does not clear them:
  1. **The CSRF token still cannot be extracted.** `http.request` returns one flat string, and while
     the Tool pack (C-115) is Rust and *could* parse a response, nothing does yet —
     [C-127](C-127-truthful-output-typing.md) owns that. A two-call handshake threading a value
     between the calls is exactly the case that does not work.
  2. **The acceptable-use question is unanswered.** flux's own design says to read Brave's ToS before
     this is more than a spike, and publishing in a *public catalogue* is a different act from one
     spike against your own room.

  The third — "the auth model assumes a credential" — is now easier to check, since C-55 and C-91
  both landed and the auth axes are better exercised.

- **The alternative is still the better connector**: an own 8x8 JaaS tenant is a paid SaaS product
  with a real credential, squarely in charter, and free of all three problems.

## Progress

- **2026-07-31 — dispatched, and parked before `providers/brave.toml` was written.** Three blockers,
  all verified by the coordinator rather than taken on the implementor's word. No provider file
  exists, so **no whole-catalogue artifact is stale and there is nothing to resolve at integration**
  — not eight red tests, not nine, zero.

  1. **This story's own Notes forbid starting.** Line 60: *"This story should not start before that
     lands"*, referring to [C-127](C-127-truthful-output-typing.md), which is still `status: ready`.
     The blocker is real: `http.request` returns one flat string and flux's `jq` parses a whole
     string as JSON, so a pointer resolves to `null` on every response — the `PUT` cannot read the
     `OPTIONS` response's `x-csrf-token` (`crates/connector-flux/src/op.rs:625-636`). A workaround
     exists — declare `x-csrf-token` as a caller-supplied required header param, as
     `providers/stripe.toml:430-435` does — but it produces exactly what the Notes pre-registered as
     the reason not to start: three operations that compile, cannot be composed, and push the
     handshake's one hard part onto the caller. **The failure mode is silent**: they pass every
     artifact check and the gap only appears when someone tries to chain them.

  2. **The acceptable-use question is unanswered.** A grep over `docs/` and `providers/` finds no ToS
     determination anywhere. Acceptance item 2 requires the answer be recorded *in the connector*, so
     it ships to consumers as fact, and the Notes say *"say so where a user will see it, or not
     ship."* That is a position on publishing a third party's free, unauthenticated endpoint into a
     public catalogue — not an implementor's call.

  3. **A new blocker, found here: [C-206](C-206-no-credential-conflates-withheld-with-absent.md).**
     Declaring no credential is legal, but the catalogue then publishes freshdesk's wording — *"has
     no safe credential configuration… live calls are disabled rather than sending a credential
     outside Flux's secret protection"* — for an endpoint where nothing is withheld and the
     unauthenticated call is correct. `status.rs:128-131` already states this distinction in a
     comment and then does not make it. C-206 blocks C-157 identically.

- **Charter question, raised and not decided.** `docs/vision.md` scopes connectors to *"paid SaaS
  services"*. Brave Talk's room-token handshake is free and unauthenticated, so it fails that test on
  its face. The implementor's recommendation is to close this in favour of an 8x8 JaaS story — a paid
  product with a real credential, a single-call auth flow, and none of blockers 1–3. Recorded rather
  than acted on: dropping a connector in favour of a different vendor is a product decision.
