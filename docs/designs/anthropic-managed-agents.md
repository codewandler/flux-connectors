# Design: Anthropic Managed Agents — the first vendor that declares both transports and its own event set

**Status:** proposed — **§The charter question gates every operation below; read it first** ·
**Pillar:** Spec (+ Codegen) · **Epic:** `anthropic-managed-agents` ·
**Extends:** [channel-bindings.md](channel-bindings.md), [inbound-events.md](inbound-events.md) ·
**Companions:** [provider-services.md](provider-services.md), [connector-contracts.md](connector-contracts.md)

> API facts below are from the bundled `claude-api` skill reference (`shared/managed-agents-*.md`),
> read 2026-08-02, **not** from a vendored document in `specs/` — none exists. Every one of them is a
> claim about a vendor, not a measurement of this repository; re-read the reference before quoting.
> Repository facts are marked separately and were measured this session.

## Why this vendor, and why now

The `channel-bindings` and `inbound-events` epics built a model for the reverse call direction —
`EventDecl`, `HmacSpec`, `VerificationScheme`, `Reply`, `ChannelBinding`, three `Transport` values —
and the catalogue has never really exercised it. **Measured 2026-08-02:** three providers declare an
inbound surface (`slack`, `stripe`, `twilio`); the whole fleet publishes **8 events and 4 channel
bindings**; `slack` is the only `socket` binding there is.

Anthropic's Managed Agents API is the first vendor in reach that would exercise **the entire model at
once, from a vendor document rather than from our invention**:

- a **socket** transport — `GET /v1/sessions/{id}/events/stream`, SSE, long-lived, that *we* open;
- a **webhook** transport — Console-registered HTTPS endpoints, HMAC-signed;
- a **closed, vendor-documented event vocabulary** on both — `agent.message`, `agent.tool_use`,
  `agent.custom_tool_use`, `session.status_idle`, `session.error`, `span.model_request_end`, … on the
  stream; a separate `data.type` namespace on the webhook side;
- a **reply operation** for the socket half — `POST /v1/sessions/{id}/events` — which is exactly what
  `AGENTS.md` demands: *"if a binding cannot answer with an operation the pipeline already emits, the
  binding is wrong, not the model."*

That last point is the reason this epic is worth more than one more provider. Slack's socket binding
was modelled from a vendor whose event set we curated by hand. Here the vendor publishes the event
set, the signature scheme, and the reply endpoint, so the binding is *derived* rather than designed.

## The charter question, which gates everything

**Do not write a single operation before this is answered.** Managed Agents is an API for creating
and running agents: a session provisions a container, runs an agent loop on Anthropic's
orchestration layer, and bills inference. That is not the ordinary SaaS read/write this repository
compiles, and it lands on two existing decisions at once:

- **[C-123](../stories/C-123-decide-connector-inference.md)** asks whether a connector may serve LLM
  inference, and is still `ready`. `vision.md`'s non-goal — *"a generated LLM-vendor connector is a
  pipeline test fixture and a convenience surface, **not the inference path**"* — was written about
  `ai.*` routing through a connector. A flow that creates a Managed Agents session does not route
  `ai.*` through a connector, but it does cause Anthropic to run inference and bill for it. **This is
  a third shape of the same question**, and C-123's Progress already records that it has been asked
  in three shapes; this is a fourth.
- **flux has its own agent layer.** `../flux/crates/flux-agent`, `flux-orchestrate` and `flux-flow`
  exist. The argument that killed connector-served inference — *"a strictly worse second
  implementation of something that already works"* — applies here with the same shape and needs to be
  answered rather than assumed away.

The honest split, and the one this design proposes:

| surface | charter reading |
|---|---|
| **Management plane** — agents, environments, vaults, memory stores, skills, deployments: CRUD over configuration objects | Ordinary SaaS. Squarely in charter, same as `anthropic`'s existing `admin` service. |
| **Session plane** — create a session, send events, stream events | **The decision.** Running a session is running an agent. |

A defensible outcome is *management-plane yes, session-plane decided separately* — which is exactly
what C-122 already did for `anthropic` (the model catalogue and admin surface shipped; inference did
not). **C-445 costed that middle and found it is not obviously safe**: the vendor's own guidance is
that agents and environments are control-plane resources applied once from CI, and calls
`agents.create()` in the request path an anti-pattern — so a management-only connector risks shipping
a catalogued, callable surface with no caller, which is the provisioning category
`provider-operation-inventory.md:717` already drops ~120 babelforce operations for. `memory_stores`
is the part that survives a "no". The inventory costs both branches and picks neither; C-444
decides. It is also possible the answer is that the session plane belongs in flux as a plugin, which
`AGENTS.md:124-130` would support: this is protocol-rich and stateful, and it holds a socket.

**This design does not settle it.** It states it, scopes the epic so the management half can proceed
independently, and files the decision as its own story.

## How it maps onto the member model

Assuming the management plane at minimum, the mapping is unusually clean — and every row below is a
member kind this repository already has:

| Managed Agents surface | member kind | notes |
|---|---|---|
| `POST/GET /v1/agents`, `/v1/environments`, `/v1/vaults`, `/v1/memory_stores` | `[[operations]]` | Plain CRUD. Note `archive` is terminal and has no inverse — `risk` must say so. |
| `GET /v1/sessions/{id}/events/stream` | **`[[channels]]`, `transport = "socket"`** | Never an operation. It is a long-lived SSE stream — `Operation` models one request and one response, and `AGENTS.md` refuses "an event dressed up as a pollable op". |
| the stream's event vocabulary | `[[events]]` | `agent.message`, `session.status_idle`, … Member names admit `.`, which is why the vendor's own spelling survives. |
| `POST /v1/sessions/{id}/events` | `[[operations]]`, and the channel's **`reply`** | The binding's outbound half. |
| Console-registered webhooks | a second `[[channels]]`, `transport = "webhook"` | Its own event namespace (`data.type`), its own verification. |

**Two channel bindings over two event sets on one service** — and **C-445 found that this does not
hold as written.** The claim was that `slack` proved the shape with `socket` + `events-api`; it did
not, because slack's two bindings share *one* vocabulary. Here the two vocabularies **collide by
name**: `session.status_terminated`, `session.status_rescheduled` and `session.thread_created` each
appear in both the SSE and the webhook set with *different payloads*, and `session.status_idle` /
`session.status_idled` differ by a single letter. A service is one member namespace
(`AGENTS.md` §Member contract), so a duplicate is a loud error rather than a merge. Resolving it is
[C-446](../stories/C-446-managed-agents-events-and-verification.md)'s, and it constrains the service
partition — which is why C-445 recorded it rather than leaving it to be discovered in the TOML.

### The socket binding needs no verification, and the webhook one does

`slack.toml:450` already records the rule: *"No `verification`: nothing arrives unsolicited over a
socket we opened and authenticated."* That holds here — the SSE stream is opened with the caller's
own API key. The webhook half is the opposite and must state an HMAC scheme or state
`verification = "none"` deliberately; silence is refused at load.

**This is where the epic earns its keep, and also where it is most likely to be refused.** The
vendor's webhook signature is a three-header scheme — `webhook-id`, `webhook-timestamp`,
`webhook-signature` — with a ~5-minute freshness window. `HmacSpec` signs a template over a closed
placeholder set (`{body}`, `{timestamp}`, `{url}`, `{sorted_form}`) with one `header` carrying the
digest, and requires `tolerance` exactly when `{timestamp}` is present. **Whether a
three-header, id-included scheme is expressible in that template is an open question and the first
thing the epic must answer** — C-141 and C-188 are the precedents for finding `HmacSpec` one axis
short. If it is not expressible, the honest output is a `verification` gap filed against `HmacSpec`,
not a hand-rolled scheme.

## What blocks it, measured 2026-08-02

- **No vendored document.** `ls specs/` → `anthropic/2023-06-01-excerpt.yaml` (3.6 KB, no Admin API,
  no Managed Agents), `babelforce/`, `flux/`, `zendesk/`. Builds are hermetic and offline, so a
  Managed Agents connector is **hand-authored**, like the `admin` service before it — and C-126's
  rule binds: a field not known with confidence is left out or left untyped, never guessed into a
  `required` list.
- **Beta, and the header goes on the operation, not the provider.** The whole surface sits behind
  `anthropic-beta: managed-agents-2026-04-01`. **Corrected by C-445:** the sentence that stood here
  said `const_headers` is *"distributed onto every operation by the loader"* and read as an argument
  for declaring it provider-level. It is distributed onto every operation — `distribute_const_headers`
  (`crates/connector-spec/src/provider.rs:2010`) loops the whole slice — which is exactly why a
  provider-level declaration is **wrong** here: it would beta-gate `anthropic-models-list`,
  `anthropic-model-get` and the three admin reads, none of which are beta. Declare it per operation;
  distribution is additive per header name, so those operations still inherit `anthropic-version`.
- **Pagination is `page`/`next_page`**, not the `after_id`/`before_id` scheme the rest of the
  Anthropic surface uses. `quirks.pagination` is declared by 6 operations across 3 providers and
  reaches no artifact (`AGENTS.md` §Intentional gaps), so declaring it here is honest and
  currently inert.
- **`providers/anthropic.toml` is under active edit** by C-441. Nothing in this epic may be
  dispatched against that file until C-441 integrates.

## Scope

**In:** the management-plane operations, both channel bindings, both event sets, the verification
conformance question, and the charter decision.

**Out:**
- **Deciding the session plane by writing it.** It is the charter story's to answer.
- **Anything streaming in this repository.** A channel binding *declares*; flux owns the socket.
  `AGENTS.md`: *"Compiling a scheduler here would make this repository a runtime, which
  `docs/vision.md` forbids."*
- **A second `[[services]]` split before the inventory.** Whether Managed Agents is one service or
  several (`agents`, `sessions`, `environments`, `vaults`, `memory`) is an inventory finding, not an
  assumption. Note the service is load-bearing for credential addressing and for `base_url`.
- **`ant` CLI, SDK bindings, self-hosted sandbox workers.** Not connector surfaces.

## Stories

Seeded with this design; see the board under the `anthropic-managed-agents` epic.

The endpoint inventory produced by [C-445](../stories/C-445-managed-agents-endpoint-inventory.md)
is [managed-agents-inventory.md](managed-agents-inventory.md) — **read its §What the inventory found
before acting on anything above**; three of its findings correct this document.
