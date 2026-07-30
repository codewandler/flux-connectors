# Design: the connectors proxy — server-side credential injection

**Status:** proposed — **and it contradicts a stated non-goal; read §Why first** ·
**Pillar:** Bridge · **Stories:** C-34 … C-36

## Why

Every credential problem in this repo has the same shape: **the caller must not hold the secret, but
the request must carry it.** So far the answer has been "flux injects it", which is the
[`$auth` seam](auth-seam.md) — a change that must land in *another repository*, on another release
cadence, and which today blocks all three providers from making a single live call.

A **connectors proxy** answers the same question differently. It is a small service that terminates a
request naming a provider and an operation, injects the credential itself, forwards to the vendor,
and returns the response. The client — a curl command, a flux op, a script — never holds a secret.

It is **provider-agnostic**: it knows nothing about Zendesk or Freshdesk specifically. It reads the
generated connector manifests, and each manifest already declares exactly what is needed — which
hosts are reachable, which credentials exist, which scheme each uses and where it goes on the
request. That is the whole point of having built the manifest.

Two things follow, and they pull in opposite directions:

**The upside is large and immediate.** It gives an execution path that does **not** depend on the
flux `$auth` seam. Generated curl examples become copy-pasteable with no secret. Credentials live in
exactly one place. Anything that speaks HTTP can use a connector, not only flux.

**The downside is that it contradicts the vision.** `vision.md` lists as a non-goal:

> **A runtime.** This repo compiles; flux executes. flux-connectors ships no server, no daemon, and
> no request path of its own.

A proxy is a server, a daemon, and a request path. **This is not a detail to be reconciled in
review — it is a deliberate change of charter, and it needs an explicit decision before any code is
written.** C-34 is that decision and nothing else in this epic should start before it resolves.

## Approach

*Everything below is conditional on C-34 deciding the charter question in favour of building it.*

### The request contract

A client addresses the proxy by provider and operation rather than by vendor URL:

```
POST /c/zendesk/zendesk-ticket-show
{ "ticket_id": 12345 }
```

The proxy resolves the operation in the connector manifest, builds the vendor request from the same
IR the Flux emitter uses, injects the credential per its declared scheme, forwards, and returns the
response. **Same IR, second backend** — the proxy and the Flux emitter must agree by construction,
or the documented curl and the executed Flux drift apart, which would be worse than having neither.

### It inherits the manifest's guarantees, and must not widen them

- **Host allowlist** — a request may only reach the `http_hosts` its manifest declares. This is the
  single most important control: without it the proxy is an open credential-lending relay, and
  anyone who can reach it can borrow a Zendesk token against a host of their choosing.
- **Credential scoping** — an operation gets the credentials its requirement set names, and no
  others.
- **Redaction** — the proxy is the only component that ever holds a plaintext credential, so it is
  the one place a leak actually matters. Credentials must never appear in logs, traces, or an error
  body returned to the client.

### The proxy must be authenticated

This is where the design earns its keep or fails. **A credential-injecting proxy is, by
construction, a confused-deputy machine**: its entire job is to add authority a caller does not have.
An unauthenticated proxy on a reachable interface is a credential-lending service for anyone who
finds it. flux learned this exact lesson — its own HTTP server refuses a non-loopback bind without a
token, on the reasoning that a daemon which auto-approves tools is remote code execution.

The same rule applies here, and more strictly, because there is no approval gate in front of it.

## Alternatives considered

- **Land the `$auth` seam in flux and ship nothing here.** The status quo. Keeps the charter intact
  and adds no attack surface, at the cost of a hard cross-repo dependency that currently blocks every
  provider.
- **Render curl with an env-var placeholder and no proxy.** Much cheaper, unblocks the documentation
  epic entirely, and is what [provider-docs](provider-docs.md) assumes today. It does not give a
  non-flux execution path — but that may simply not be a goal.
- **Build the proxy in a separate repository.** Keeps this repo's charter clean and lets the proxy
  have its own security posture and release cadence. **This is the option to weigh most seriously
  against building it here** — the connector manifest is a public artifact, so a proxy consuming it
  needs no privileged access to this codebase.
- **Do it as a flux plugin.** Reuses flux's existing capability gating, but reintroduces the
  per-integration binary this project exists to eliminate.

## Risks & open questions

- **The charter conflict is unresolved and blocks everything.** C-34.
- **Blast radius.** This is the first component in the project that holds plaintext credentials at
  runtime. Every other artifact here is inert text. That is a categorical change in what a
  vulnerability costs.
- **Two backends, one IR — they will drift.** If the proxy builds requests differently from the Flux
  emitter, the documented curl stops matching the executed operation. A shared request-builder and a
  conformance test are the only real defence.
- **It could quietly become the primary execution path**, making flux optional and inverting the
  project's relationship to it. That may be desirable; it should not happen by accident.
- **Percent-encoding and body-shape gaps apply here too** — the proxy inherits C-29 and C-30 rather
  than escaping them.

## Acceptance / done

- **C-34 decides the charter question**, and either `vision.md`'s non-goal is amended or this epic is
  closed as out of scope. Nothing else starts first.
- If built: a request naming provider + operation is forwarded with its credential injected, and a
  request for a host outside the manifest's allowlist is refused.
- The proxy is authenticated, and refuses a non-loopback bind without a token.
- No credential appears in any log, trace, or returned error.
- A conformance test proves the proxy and the Flux emitter build the same vendor request from the
  same IR.
