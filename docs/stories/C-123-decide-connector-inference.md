---
id: C-123
title: "Decide: may a connector ever serve LLM inference?"
pillar: Spec
status: ready
priority: 4
design: docs/designs/provider-roles.md
epic: provider-roles
areas: [connector-spec, bridge]
note: "DECISION, not a task — it contradicts a stated vision non-goal, so it needs an explicit answer before any code, exactly as C-34 gates the proxy. Nothing else in the epic depends on it"
---

# Decide: may a connector ever serve LLM inference?

## Goal

Answer one question, in writing, and record it: **should a connector-declared service ever be a
source of LLM inference**, contributing an `llm_inference` role that `ai.*` can route to — rather than
only contributing the model catalogue?

This is a charter decision. It is filed so the option stays open and honest instead of being
half-built toward, and it produces a written answer rather than code.

## Acceptance

- [ ] A decision is recorded in [provider-roles.md](../designs/provider-roles.md) with its reasoning,
      and this story closes as `done` **whichever way it goes**. A "no" is a successful outcome.
- [ ] If **no**: `vision.md`'s non-goal is left standing and, if the wording needs sharpening after
      this analysis, it is sharpened. `llm_inference` is not added to the closed role set.
- [ ] If **yes**: `vision.md` is amended explicitly — a non-goal that is quietly outgrown is worse
      than one that is changed on purpose — and follow-up stories are filed for streaming, native tool
      calling, prompt caching and usage/cost, none of which a generated operation models today.

## The case against, as it stands

- **`vision.md` says no**: *"Replacing flux's native model providers… A generated LLM-vendor connector
  is a pipeline test fixture and a convenience surface, not the inference path."*
- **A connector operation is one request and one response.** Inference needs SSE streaming
  (`flux-provider`'s `WireCodec`), the native tool-calling loop, prompt caching per provider profile,
  and usage/cost accounting.
- **The cost accounting is genuinely vendor-specific truth.** `openrouter_reported_cost` in
  `crates/flux-providers/src/lib.rs` exists because a live probe found that for non-BYOK calls
  `upstream_inference_cost` *duplicates* `cost`, so summing unconditionally double-counts.
- **C-403 closed the old flat-string limitation.** `http.request` now returns
  `{status, headers, body}`, so field selection is no longer evidence against connector-served
  inference. Streaming, the native tool loop, caching and usage/cost remain independent objections.
- **flux already has native providers** for `openai`, `openrouter`, `anthropic`, `codex`, `ollama` and
  `bedrock`. A connector path would be a second, worse implementation of a solved problem.

## The case for, stated fairly

- A long tail of OpenAI-compatible vendors exists that flux will never hand-write a provider for, and
  a declarative connector is exactly how a long tail gets covered.
- The [C-113](C-113-tool-pack-epic.md) Tool pack changes the calculus: a Tool is Rust, so it *could*
  implement `flux_provider`'s `Provider`/`WireCodec` traits rather than being limited to what
  `http.request` can express. That was not true when the non-goal was written.
- If the answer is driven by "generated ops can't stream", note that the constraint is about the
  *composite* path, and re-check it against the Tool pack before treating it as settled.

## Notes

- Do not start any `llm_inference` implementation before this closes. That is the whole point of the
  story, and it is the same discipline [C-34](C-34-decide-proxy-charter.md) applies to the proxy.
- The decision should name **who** it binds: this repo's charter, not flux's roadmap.

## Progress

- **This question has now been asked three times, in three shapes**, which is why it is worth one
  written answer rather than three ad-hoc ones:

  1. The LLM pool ([C-119](C-119-provider-roles-epic.md)) — resolved by splitting at the plane boundary:
     connectors inform the pool, flux serves it.
  2. This story — may a connector *serve* inference at all.
  3. [C-157](C-157-ollama-model-catalogue.md) — an ollama inference provider choosable as `ollama/…`.

  C-157 turned out to be **already shipped**: `flux-providers`' `KNOWN_PROVIDERS` (`spec.rs:16-25`)
  already lists `"ollama"` and `"ollama-anthropic"`, so `ollama/llama3` resolves today. What is missing
  there is only *discovery* of locally installed models.

- **C-495 resolves C-157's repository axis.** Ollama discovery may be a connector even though it is a
  local process. This story now decides only the independent inference-plane question: whether a
  connector should duplicate Flux's native provider and agent loop.
