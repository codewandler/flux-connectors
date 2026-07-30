# Design: provider roles — a declared, checkable capability shape

**Status:** proposed — **one half contradicts a stated non-goal; read §The split first** ·
**Pillar:** Spec · **Stories:** [C-119](../stories/C-119-provider-roles-epic.md) … C-123

> Citations into `../flux` were read at `codewandler-flux-lang` 0.39.0. Re-grep by symbol; line
> numbers move.

## Why

Seventeen connectors share structure that nothing currently names. `zendesk`, `freshdesk`,
`intercom` and `jira` are all ticketing surfaces with a *show*, a *search*, a *comment list*. `openai`
and `openrouter` both list models and both run a completion. Nothing in the IR says so, so nothing
can act on it: a UI cannot group them, a flow cannot ask "who can do this", and a second vendor
filling the same shape is a coincidence rather than a contract.

A **role** is that missing declaration: a named shape a service claims to implement, checked at load.

## Roles attach to a service, not to a provider

This is the correction the idea needs, and the existing model already made room for it.

`openai` is not "an LLM provider". It exposes a **management** surface (`openai-models-list`,
`openai-model-get`) and an **inference** surface (`openai-chat-completion`) — different capabilities,
different risk, different idempotency, same vendor. C-49's `provider → service → members` level is
exactly the right granularity, and a role declared on the provider would smear the two together.

```toml
[[services]]
name = "models"
roles = ["llm_catalogue"]

[[services]]
name = "chat"
roles = ["llm_inference"]
```

A provider's roles are then *derived* — the union of its services' — never authored. Same rule as
`Level` in [connector-configuration.md](connector-configuration.md), which is derived from `binds`
and never written by hand.

## A role is a contract, and every rule is a refusal

A role names required operations by their **member name** within the service, with an agreed shape.
Declaring a role you do not satisfy is a load error, in the tradition every other rule here follows:

- A service claiming `llm_catalogue` without a `list` operation is refused.
- A required operation whose declared parameters cannot satisfy the role's shape is refused, naming
  the missing one.
- An unknown role name is refused rather than ignored — a typo'd capability that silently means
  "no capability" is the failure mode this whole design exists to prevent.
- Roles are a **closed set** defined in this repo. An open string set is a tag system, and a tag
  system cannot be checked.

The point is that `llm_catalogue` becomes a *promise the loader enforces*, so a consumer reading the
catalogue can rely on it without reading the provider's TOML.

## The split: catalogue yes, inference no

The request was that LLM providers "contribute to the list of llm inference services + models", with
`ai.*` then resolving a `(provider, model)` tuple from that pool. **Half of that is a clean win and
half of it contradicts `vision.md`, which lists as a non-goal:**

> **Replacing flux's native model providers.** flux talks to Anthropic and friends through
> `flux-providers`. A generated LLM-vendor connector is a pipeline test fixture and a convenience
> surface, **not the inference path**.

That is not a technicality to reconcile in review. It is also, independently, the right engineering
call — and the reason is concrete rather than doctrinal.

### Why a connector cannot serve inference

A connector operation is **one request, one response**. Inference is not:

- **Streaming.** `flux-provider`'s `WireCodec` (`crates/flux-provider/src/lib.rs:217`) maps SSE frames
  into events. A generated op has no streaming model at all.
- **Native tool calling.** The loop that turns a tool-use block into a follow-up request lives in the
  provider layer.
- **Prompt caching.** `flux-providers`' module doc records this per provider profile, e.g. caching
  for `anthropic/…` slugs on the OpenRouter wire.
- **Usage and cost.** `openrouter_reported_cost` in `crates/flux-providers/src/lib.rs` exists because
  a live probe found that for non-BYOK calls `upstream_inference_cost` *duplicates* `cost`, so summing
  unconditionally double-counts. That is the level of vendor-specific truth the inference path
  carries.

And for the two vendors named, **flux already has hand-written native providers** — `openai`,
`openrouter`, `anthropic`, `codex`, `ollama`, `bedrock`. A connector-served inference path would be a
strictly worse second implementation of something that already works.

`http.request` returning one flat string (`HTTP {status}\n{headers}\n{body}`) — the constraint
`crates/connector-flux/src/op.rs` already records — settles it on its own: an emitted connector op
cannot even field-select a completion out of its own response today.

### What connectors *should* contribute: the model pool

The genuinely useful half is the one the vision permits, and it fills a real gap.

flux's model metadata — context windows, pricing — lives in **static tables** inside each provider
module, with `openrouter_reported_cost` noting that "the static pricing table stays the fallback".
Static tables go stale the moment a vendor ships a model.

`openai-models-list` is *live*. A service with the `llm_catalogue` role contributes **what models
exist**, discovered rather than tabled. So:

- **Connectors inform the pool** — model ids, and later context window and pricing.
- **flux serves the pool** — `ai.extract | ai.judge | ai.rank | ai.reason | ai.rewrite` resolve a
  `(provider, model)` tuple through `flux-providers`' existing `provider/model` spec resolution, which
  `crates/flux-providers/src/spec.rs` already centralises so "a spec resolves identically everywhere".

The user's mental model survives intact — one pool of `(provider, model)` tuples, contributed to by
registered providers. Only the *serving* half stays where it already works.

> The `ai.*` family is `extract`, `judge`, `rank`, `reason`, `rewrite` — verified in
> `crates/flux-tools` and `crates/flux-cognition`. There is no `ai.summarize`, `ai.shorten` or
> `ai.map` today; if those are wanted they are flux stories, not connector ones.

## The inference question is a charter decision, not a task

If the intent really is that connectors serve inference, that is a deliberate change of charter and
needs deciding before any code — exactly as [C-34](../stories/C-34-decide-proxy-charter.md) gates
[connectors-proxy.md](connectors-proxy.md). **C-123 is that decision and nothing else in this epic
depends on it.** Filing it keeps the option open and honest instead of half-building toward it.

## Roles worth defining first

Start with two, because two is what the shipped catalogue can actually check:

| role | required members | shipped candidates |
|---|---|---|
| `llm_catalogue` | `list` (models), optional `get` | `openai`, `openrouter`, + `anthropic` (C-122) |
| `ticketing` | `show`, `search`, `comment.list` | `zendesk`, `freshdesk`, `intercom`, `jira` |

`ticketing` is included deliberately: a role mechanism validated by exactly one role is a mechanism
designed around one case. Four shipped providers already fit this shape, and flux's retained
`examples/zendesk.triage.flux` is written against precisely those three operations — so the role has a
real consumer on day one.

## Out of scope

- **Serving inference from a connector** — C-123's decision, and a non-goal until it says otherwise.
- **Pricing and context windows in the pool.** Model *ids* first; the richer metadata is a follow-up
  once one role is proven, and it is where vendor disagreement will actually bite.
- **A role hierarchy or role inheritance.** Closed, flat, checkable. If that proves too weak, widening
  it later is cheap; narrowing an open system is not.
