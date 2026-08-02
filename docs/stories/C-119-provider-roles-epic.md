---
id: C-119
title: "Provider roles — a declared, checkable capability shape (epic)"
pillar: Spec
status: ready
priority: 2
design: docs/designs/provider-roles.md
epic: provider-roles
areas: [connector-spec, codegen, providers]
note: "EPIC — a role is a contract the loader enforces, declared on a SERVICE not a provider (openai's models surface and its chat surface are different capabilities). Connectors inform the model pool; flux keeps serving inference, per the vision non-goal"
---

# Provider roles — a declared, checkable capability shape (epic)

## Goal

Let a service **declare a named shape it implements**, checked at load, so that a second vendor
filling the same shape is a contract rather than a coincidence.

Seventeen connectors already share structure nothing names: four are ticketing surfaces with a show,
a search and a comment list; two list models and run completions. A role makes that structure
queryable — by a UI grouping providers, by a flow asking "who can do this", and by the model pool.

## Acceptance

- [x] A **service** declares `roles = [...]`. A provider's roles are *derived* as the union of its
      services', never authored — the same rule `Level` already follows in
      [connector-configuration.md](../designs/connector-configuration.md).
- [x] Roles are a **closed set** defined in this repo. An unknown role name is refused at load, not
      ignored: a typo'd capability that silently means "no capability" is the failure this epic exists
      to prevent.
- [x] **Every rule is a refusal.** A service claiming a role without the role's required members is
      refused, naming the missing one. So is one whose declared parameters cannot satisfy the shape.
- [ ] Two roles ship, not one: `llm_catalogue` and `ticketing`. A mechanism validated by a single role
      is a mechanism designed around a single case.
- [ ] Roles reach the manifest and `catalog.json`, so a consumer relies on the promise without reading
      provider TOML.
- [ ] The model pool is **informed by connectors and served by flux**: a `llm_catalogue` service
      contributes model ids; `ai.extract|judge|rank|reason|rewrite` keep resolving `(provider, model)`
      through `flux-providers`' existing spec resolution.

## Children

- [C-120](C-120-service-roles-declaration.md) — the `roles` declaration, the closed set, the refusals
- [C-121](C-121-llm-catalogue-role.md) — the `llm_catalogue` role and the model-pool projection
- [C-122](C-122-provider-anthropic.md) — the Anthropic connector, a second vendor filling the role
- [C-123](C-123-decide-connector-inference.md) — **decision**: may a connector ever serve inference?

## Notes

**The vision non-goal this epic respects.** `vision.md`: *"Replacing flux's native model providers.
flux talks to Anthropic and friends through `flux-providers`. A generated LLM-vendor connector is a
pipeline test fixture and a convenience surface, **not the inference path**."*

That is also the right engineering call independently. A connector operation is one request and one
response; inference needs SSE streaming, native tool calling, prompt caching and usage/cost
accounting. C-403's response record closed the older field-selection objection, but none of those
streaming or provider-loop gaps. flux already has hand-written native providers for `openai`,
`openrouter`, `anthropic`, `codex`, `ollama` and `bedrock`.

**What connectors genuinely add** is the *live* catalogue. flux's model metadata sits in static tables
inside each provider module, with the pricing table explicitly "the fallback". `openai-models-list` is
live. That is the gap worth filling.

**If the intent really is connector-served inference**, that is a change of charter, not a task —
C-123 decides it, exactly as [C-34](C-34-decide-proxy-charter.md) gates the proxy. Nothing else here
depends on that answer.
