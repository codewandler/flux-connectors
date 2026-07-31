---
id: C-133
title: "The brave connector — Brave Talk's room-token HTTP surface"
pillar: Spec
status: blocked
priority: 5
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

### 2026-07-31 — dispatched for implementation, returned blocked. No TOML was written.

An implementor picked this up and stopped before authoring `providers/brave.toml`. All three
blockers were re-checked against the tree at `e350ed5`; **all three still stand**, and the third is
now answered in the negative rather than merely open.

**1 · The CSRF token still cannot be extracted — verified in code.**
`crates/connector-flux/src/op.rs:625-636` records the constraint directly: `http.request` returns
one flat string, flux's `jq` parses a *whole* string as JSON, so a pointer applied to that string
"resolves to `null` on every response, success or failure". The `PUT` therefore cannot read the
`OPTIONS` response's `x-csrf-token`. [C-127](C-127-truthful-output-typing.md), which this story names
as the fix, is still `status: ready` — not landed. The Notes above say in terms that **this story
should not start before that lands**, and that instruction was followed.

The workaround available today is to declare `x-csrf-token` as a caller-supplied required header
param on the `PUT` (the loader and emitter permit it — cf. `providers/stripe.toml:430-435`). That
compiles, and it produces precisely the outcome the Notes pre-registered as the reason not to start:
three operations that compile and cannot be composed, with the handshake's one hard part pushed onto
the caller.

**2 · "No credential" is expressible, but the catalogue publishes it as a defect, and for Brave that
defect notice would be false.** This is the question Note 2 raises and that
[C-157](C-157-ollama-model-catalogue.md) asks to be settled once for both. Settling it:

- The IR permits zero `[[auth]]` and no `default_auth`; `validate_credentials`
  (`crates/connector-spec/src/provider.rs:2029-2089`) passes vacuously, and
  `crates/connector-spec/tests/auth_archetypes.rs:162-173` pins the shape as archetype 4.
- But `crates/connector-cli/src/status.rs:133-143` then emits a `no-credential` issue whose published
  summary reads: *"<id> has no safe credential configuration for this operation yet. Live calls are
  disabled rather than sending a credential outside Flux's secret protection."*

That sentence is true of freshdesk — a real API key exists and is deliberately withheld, so the 401
is fail-closed and honest. **It is false of Brave**, where no credential exists at all, nothing is
being withheld, and an unauthenticated call is the correct and working call. The catalogue would tell
consumers a public endpoint is disabled for their protection.

The code already knows these are two different things. `status.rs:128-131` notes that an operation
declaring nothing inherits the default while one declaring an explicit empty list inherits nothing,
"reporting freshdesk and a genuine ping endpoint the same way for opposite reasons" — and then
`effective_auth` collapses both into the one `NO_CREDENTIAL` code with freshdesk's wording anyway. A
genuinely-public endpoint is exactly the case that gets mislabelled.

**So the answer to Note 2 is: not today.** `no-credential` needs to split into "withheld because it
cannot be placed safely" (freshdesk) and "none exists; the endpoint is public" (brave, ollama) before
either connector can describe itself honestly in the catalogue. `NO_CREDENTIAL` is a published
contract token (`status.rs:64-70`: consumers switch on these and they are not renamed once shipped),
so this is an additive-code change and wants its own story. **This blocks C-157 identically.**

**3 · The acceptable-use question is still unanswered, and an implementor cannot answer it.** A grep
over `docs/` and `providers/` finds no ToS determination anywhere in the repository — the only
mentions of Brave are this story and C-157's cross-reference. Acceptance item 2 requires the answer
to be **recorded in the connector**, i.e. shipped to consumers as a statement of fact, and the Notes
say the connector should say so where a user will see it **or not ship**. Answering requires reading
Brave's Terms and taking a position on publishing a third party's free, unauthenticated endpoint into
a public catalogue — the act this story itself calls "meaningfully different" from one spike, because
it "invites exactly the at-scale use that warning is about". That is a human decision with an
external party on the other side of it; guessing it and writing the guess into a shipped artifact
would be the worst available outcome.

**Charter note, unchanged and worth restating.** `docs/vision.md:83` — "Connectors are **paid SaaS
services**." The Brave Talk handshake is free and unauthenticated, so it fails the charter test on
its face. The story's own conclusion still holds and no longer has a competing reading: **the 8x8
JaaS connector is the one to build** — a paid product, a real credential, a composable single-call
auth flow, and none of blockers 1–3. Recommend closing this story in favour of a JaaS story unless
someone actively wants the free-tier surface.
