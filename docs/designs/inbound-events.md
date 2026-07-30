# Design: inbound events — the reverse call direction

**Status:** proposed · **Pillar:** Spec (+ Codegen, Bridge) · **Epic:** `inbound-events` ·
**Stories:** C-58 … C-65

## Why

A connector today compiles a vendor spec into **outbound** ops: flux calls Zendesk, GitHub, Slack.
That is one half of an integration. The other half is the vendor calling **us** — a ticket was
updated, a call ended, a payment settled, a message was posted. Without it, every connector-driven
automation has to poll, which is slower, costs quota, and cannot express "react when this happens."

So a connector must define **both directions**: the operations we invoke, *and* the events we receive.
An integration that only knows how to make calls is not an integration; it is an API client.

The asymmetry is not merely missing plumbing — it is the reason the [Zendesk
automation](../../flux/docs/designs/zendesk-automation.md) work stalled at "read the ticket when a
human asks" instead of "triage the ticket when it arrives."

## What this must not become

The [vision](../vision.md)'s non-goals are load-bearing here, and inbound is exactly where a repo like
this one would be tempted to violate them:

- **No runtime.** flux-connectors ships no server, no daemon, and no request path. It does **not**
  host an endpoint, run a relay, or offer a tunnel. Inbound must *compile* into artifacts flux runs,
  the same as ops do.
- **No homegrown DSL and no interpretation.** Verification and routing are **declared** in the spec
  and **generated**, never a per-vendor script the runtime reads at request time.
- **No unified event taxonomy.** Events keep their vendor names (`github.issues.opened`), exactly as
  ops keep vendor operation names. A normalized cross-vendor event model is a different product.
- **No event storage.** flux's event bus and event store already own delivery, replay and audit.

## The five things inbound actually consists of

Every vendor differs, but every vendor's webhook decomposes into the same five concerns. Naming them
is what turns "support webhooks" into a bounded compiler problem.

1. **Transport** — how the event arrives. Overwhelmingly an HTTP POST; sometimes an outbound socket
   (Slack Socket Mode); sometimes nothing at all, and polling is the only option.
2. **Subscription** — most vendors require you to *register* the endpoint through their API, often
   with a secret you generate. This is an **ordinary outbound op** and therefore already inside the
   existing pipeline's competence — it just has to be selected and emitted.
3. **Verification** — proving the request came from the vendor. This is the crux, and section below.
4. **Identity and payload** — which event this is (a body field or a header) and what its payload
   looks like, so a trigger receives typed data rather than an opaque blob.
5. **Replay and idempotency** — vendors redeliver. A delivery id must reach the flow, and timestamped
   schemes must bound how old a request may be.

## Verification is a declarable matrix, not per-vendor code

This is the finding that makes the whole design fit the north star. Vendor webhook signatures look
bespoke but vary along a small, fixed set of axes:

| vendor | header | algorithm | encoding | what is signed | window |
|---|---|---|---|---|---|
| GitHub | `X-Hub-Signature-256` | HMAC-SHA256 | hex, `sha256=` prefix | raw body | — |
| Stripe | `Stripe-Signature` | HMAC-SHA256 | hex, `t=`/`v1=` pairs | `{t}.{body}` | tolerance |
| Slack | `X-Slack-Signature` | HMAC-SHA256 | hex, `v0=` prefix | `v0:{ts}:{body}` | 5 min |
| Zendesk | `X-Zendesk-Webhook-Signature` | HMAC-SHA256 | base64 | `{ts}{body}` | timestamp header |

Four vendors, four "unique" schemes, **one** parameterized algorithm: pick a digest, pick an encoding,
build the signed string from a template over `body`/`timestamp`, compare in constant time, optionally
enforce a tolerance. That is a struct, not a script — so it belongs in the IR and can be generated,
which is precisely what principle 2 ("no homegrown DSL") demands.

**This is the same problem [C-50](../stories/C-50-aws-services.md) found from the other side.** AWS
SigV4 *signs* an outbound request from its bytes; a webhook scheme *verifies* an inbound request from
its bytes. Both are request-dependent, which is exactly what `source × acquisition × placement`
([unified-auth.md](unified-auth.md), C-19) cannot express. One notion should cover both directions
rather than this design inventing a second — a constraint [C-66](../stories/C-66-members-under-services.md)
states as acceptance, and which the IR shape above must be reconciled against before C-59 ships.

## Shape — a `[inbound]` section in the provider TOML

```toml
[inbound]
transport = "webhook"

[inbound.verification]
scheme     = "hmac"
algorithm  = "sha256"
encoding   = "hex"            # hex | base64
header     = "X-Hub-Signature-256"
prefix     = "sha256="        # optional literal prefix
signed     = "{body}"         # template over {body} / {timestamp}
secret     = "webhook_secret" # a CREDENTIAL NAME, resolved by the host — never a value
tolerance  = "5m"             # optional; required when {timestamp} is in `signed`

[inbound.discriminator]
source = "header"             # header | body
name   = "X-GitHub-Event"

[inbound.delivery_id]
source = "header"
name   = "X-GitHub-Delivery"

[[inbound.event]]
name   = "issues.opened"
when   = { action = "opened" }              # narrows a coarse vendor event
schema = "#/components/schemas/IssuesEvent" # from the vendored spec, when it publishes one
```

Note what is *absent*: no code, no secret, no URL. The endpoint address is the operator's deployment
detail, not a build-time fact, and the secret is a reference the host resolves — the same asymmetry
principle 5 states for outbound credentials.

## Codegen — what a connector emits for inbound

Into the existing artifacts, so there is no new artifact kind — but the split between them is strict:

1. **A manifest `[inbound]` block** — the event name, direction, transport, payload schema, the
   verification scheme's parameters, and the **credential name** the host must supply. It **declares**;
   it does not self-install. An operator wires the endpoint deliberately.
2. **Subscription ops** in the module — `<name>_webhook_subscribe` / `_unsubscribe` / `_list`, generated
   from the vendor spec like any other op. Registering a webhook is therefore an ordinary authorized,
   approvable write, not a special build-time side effect.
3. **Nothing in the module for the event itself.**

Point 3 is a correction worth stating explicitly, because the obvious design is wrong. A connector
module is loaded from `~/.flux/flows` and flux **lifts `op` declarations only**
([connector-pipeline.md](connector-pipeline.md)). `channel` and `trigger` are **Program** members that
an operator declares in their app file — so a generated `channel`/`trigger`/`event` construct inside a
connector module would be dead text that flux silently ignores. Worse, the tempting workaround —
emitting an event as a *pollable op* so that it looks callable — is precisely the plausible-but-wrong
output `AGENTS.md` requires the emitter to refuse.

What crosses into flux is therefore not code but **parameters**: the verification scheme a program's
`channel webhook` declaration consumes (see the seam below). [C-66](../stories/C-66-members-under-services.md)
reached the same conclusion independently from the member-kind side, and owns the model question of how
a channel sits under a service alongside operations.

## The flux-side seam (the blocking cross-repo change)

flux's `webhook` channel today authenticates with an **optional static bearer token** and performs no
signature verification (`crates/flux-channels/src/adapters/webhook.rs` — verified: no HMAC path
exists). A vendor that signs its payloads but cannot send a custom `Authorization` header therefore
has *no authenticated route into flux at all*. Generated verification has nowhere to run.

So inbound needs flux to gain, mirroring how [C-16](../stories/C-16-design-auth-seam.md) handled the
outbound `$auth` seam:

1. **A declarative `verify` block on `channel webhook`** — the same axes as the IR above, resolved
   from a host credential.
2. **Verification over the raw body, before parsing.** This is the classic implementation bug:
   verifying a re-serialized body fails on byte-identical-but-differently-ordered JSON, and any
   "normalize then verify" step is a bypass. The raw bytes are the message.
3. **Constant-time comparison** and **timestamp tolerance**, so a leaked signature is not replayable
   forever and comparison does not leak via timing.
4. **Discriminator → trigger label routing**, so `trigger on "github.issues.opened"` is possible
   instead of one mega-trigger switching on JSON inside the flow.
5. **A challenge/handshake hook** — Slack's `url_verification` echo, Meta's `hub.challenge` GET —
   answered by the channel *without waking an agent*. An endpoint-verification request is not a turn.
6. **Delivery id in the payload**, so a flow can dedupe redeliveries.

Per repo convention these are drafted as ready-to-paste flux stories in
[inbound-events-flux-stories.md](inbound-events-flux-stories.md), which is a **handoff artifact, not
this board's backlog**.

## Polling — the fallback that needs no new primitive

For vendors with no webhook at all, a connector may declare `transport = "poll"` with a cursor op and an
interval. Two artifacts follow, under the same strict split as above: the **cursor op** is emitted into
the module (an ordinary operation, entirely legitimate), and the loop is a **documented program pattern**
— an operator's `channel schedule` + `trigger` — not a journey emitted into the module. Durable cursor
state lives in the flow's store on flux's side; this repo ships no runtime to hold it.

The consumer-visible surface stays the **same** as the webhook case: a trigger label and a typed payload.
Worth building second, because it validates that "inbound" is an abstraction over transports rather than
a synonym for "webhook", and it ships with **zero flux-side blockers** — it can land while the seam below
is still open.

## Invariants (verify before ship)

1. **Verification is over raw bytes, before parsing** — or the connector declares it cannot verify and
   the generated artifact says so loudly. Never present an unverified event as trusted.
2. **Fail closed.** A missing, malformed, or mismatched signature yields a 401/403 and **no delivery**.
   No agent, journey, or model call is reached. Test asserts the delivery count is zero, not merely
   that the response was an error.
3. **The webhook secret never lands in an artifact** — not in the provider TOML, not in the generated
   `.flux`, not in the manifest, not in the lockfile. It is a credential name resolved by the host and
   registered with the redactor (principle 4).
4. **Verification is generated from a declared scheme**, never hand-written per vendor. A conformance
   matrix with **real signature vectors** from each vendor's documentation is the test.
5. **Replay is bounded.** Any scheme whose `signed` template includes `{timestamp}` enforces
   `tolerance`; the delivery id reaches the payload so flows can dedupe at-least-once delivery.
6. **Subscription is an ordinary op**, traversing the same authorization → approval → guarded-IO
   envelope as any other write. Registering a webhook is never a build-time side effect.
7. **Drift detection covers inbound.** If a vendor's event schema moves upstream,
   `flux-connectors check` fails exactly as it does for operations.

## Alternatives considered

- **Ship a small inbound relay in this repo.** Solves local development and NAT in one stroke, and is
  the single most tempting option — rejected because it makes flux-connectors a runtime, contradicting
  the north star and duplicating a request path flux already owns.
- **Leave inbound entirely to flux, with no spec involvement.** Then every operator hand-writes the
  verification for each vendor, which is precisely the hand-maintained integration config principle 1
  exists to eliminate — and the vendor already documents the scheme.
- **A generic "signed webhook" primitive in flux with no connector-side declaration.** Half-measure:
  flux would carry the mechanism while the per-vendor parameters stayed tribal knowledge in operator
  config, undetected by drift checks.
- **Normalize events into a unified taxonomy.** Rejected as a non-goal; it discards vendor fidelity
  and invents semantics the vendor never promised.

## Open questions

- **Does flux gain a first-class `event` declaration**, or do generated events ride the existing
  `channel webhook` + `trigger` pair with a routing table? The latter needs no language change and is
  the assumed starting point; the former is cleaner if inbound proves central.
- **Slack Socket Mode** is a second transport flux already implements as its own channel kind. Does a
  connector's inbound declaration target it, or does Slack stay special-cased? Probably the latter
  until a second socket-mode vendor appears.
- **Endpoint lifecycle ownership.** If a connector emits `subscribe`, who calls it — an operator
  running a setup flow once, or a program at startup that reconciles its own subscriptions? The second
  is more autonomous and more dangerous (a restart loop creating duplicate webhooks).
