---
id: C-157
title: "Ollama inference already works — what is missing is knowing which models are installed"
pillar: Spec
status: ready
priority: 4
design: docs/designs/provider-roles.md
epic: provider-roles
areas: [providers, connector-spec]
note: "flux's KNOWN_PROVIDERS already contains \"ollama\" and \"ollama-anthropic\", so ollama/llama3 resolves today. Nothing enumerates the LOCAL models though — GET /api/tags is one request, one response, so discovery is the connector-shaped half"
---

# Ollama inference already works — what is missing is knowing which models are installed

## What already exists, with the receipts

The request was an ollama inference provider whose models can be chosen from a global list as
`ollama/…`. **That ships today, in flux.** Three facts, read rather than recalled:

- `crates/flux-providers/src/lib.rs:14` — *"[`ollama`] — the `ollama-anthropic` provider (local models
  over the Messages protocol)"*, and `pub mod ollama` at `:32`.
- `crates/flux-providers/src/spec.rs:16-25` — `KNOWN_PROVIDERS`, documented as *"the providers a model
  spec may name"*, already lists **both** `"ollama"` and `"ollama-anthropic"`.
- `spec.rs:200` — `"ollama" => crate::openai::ollama_api()`.

So `ollama/llama3` already resolves, from the one place `flux-providers` centralises resolution *"so a
spec resolves identically everywhere"*. There is nothing for this repository to add to the inference
path, and adding it would be a second, worse implementation of something that works.

## What is genuinely missing

**Nothing enumerates the models actually installed on the machine.** `grep` for `api/tags`,
`list_models`, `available_models` across `flux-providers` returns nothing. A user must already know
what they have pulled; the global list names the *provider*, not its models.

Ollama exposes `GET /api/tags`, which returns the installed model list. That is **one request and one
response** — exactly the shape this repository compiles, unlike inference, which needs SSE streaming
and a tool-calling loop ([C-123](C-123-decide-connector-inference.md) records why that stays in flux).

And it is exactly the split [C-119](C-119-provider-roles-epic.md) designed: **connectors inform the
pool, flux serves it.** `openai-models-list` is already the worked example of live discovery beating
flux's static tables; ollama is the same shape, for the one provider whose model set is different on
every machine.

## The repository question is answered; the inference question is not

C-495/C-496 explicitly admit local processes and protocol-rich systems as connectors, so Ollama
discovery belongs here. C-123 still decides the independent question of whether connector execution
should duplicate Flux's native inference provider; this story continues to say no inference and does
not need that decision before shipping discovery.

## Acceptance, conditional on that decision going in favour

- [ ] `providers/ollama.toml` with a **catalogue service only** — `GET /api/tags`, and optionally
      `GET /api/show` for one model's detail. **No inference operation**: `/api/generate` and
      `/api/chat` stay with flux's native provider, and a test asserts neither appears.
- [ ] It claims `llm_catalogue` ([C-121](C-121-llm-catalogue-role.md)), so it joins the model pool by
      the same mechanism as openai and openrouter rather than a special case.
- [ ] **The base URL is an operator's own host.** Ollama defaults to `http://localhost:11434`, which is
      a templated/config-bound host, so this inherits C-10's gap — 27 of 105 operations already cannot
      reach a vendor for want of base-URL config. Say so; do not imply it works without it.
- [ ] **No credential.** Ollama is unauthenticated by default. Whether the auth model can express "no
      credential" without a consumer reading it as a mistake is the same open question
      [C-133](C-133-provider-brave-talk-tokens.md) records — settle it once, for both.
- [ ] `http` rather than `https` in a declared base URL is a first for this repo. Check whether the
      loader, `http_hosts`, and flux's SSRF guard accept a loopback plaintext host at all — a connector
      that cannot be called is worse than one that is absent.
- [ ] Generated Flux parses, analyzes and is a fixed point of flux's own formatter.
- [ ] The build stays a fixed point and the gate is green.

## Notes

- **Do not add inference.** Not because it is hard, but because it is done: flux's `ollama-anthropic`
  provider speaks the Messages protocol, handles streaming, and is already in `KNOWN_PROVIDERS`. A
  connector-served `ollama/…` would shadow a working path with a worse one.
- The genuinely valuable end state is the one C-121 sketches: a `(provider, model)` pool that is **live**
  rather than tabled. Ollama is the strongest case for it, because its model set is not merely stale in
  a static table — it is *unknowable* from one, differing per machine.
- C-495 makes this connector the eventual discovery home. Flux's existing Ollama provider remains
  the inference implementation unless C-123 separately decides otherwise.
