---
id: C-51
title: Ship the OpenAI connector
pillar: Spec
status: ready
priority: 3
design: docs/designs/provider-operation-inventory.md
epic: connectors-v1
areas: [providers, connector-spec]
note: bearer · JSON in and out · no query strings
---

# Ship the OpenAI connector

## Goal
Add `providers/openai.toml` and its generated artifacts: a curated, immediately useful connector for
the API that `AGENTS.md` already names as in-charter, and the first connector whose whole surface is
JSON in and JSON out with no query string at all.

## Acceptance
- [ ] `providers/openai.toml` is hand-authored — OpenAI publishes an OpenAPI document but C-4's
      ingest is not implemented, so this follows the zendesk precedent (`[spec]` pointer noted as the
      later form, operation set as the selection to reproduce).
- [ ] `base_url = "https://api.openai.com"`, `vendor = "OpenAI"`, and a `[[auth]]` entry with
      `scheme = "bearer"` over `OPENAI_API_KEY`. `default_auth` names it for every operation.
- [ ] A curated operation set of roughly four, each with `risk` and `idempotency`. Confirm the
      shapes against current vendor docs before authoring; the intended set is
      `openai-models-list` (GET `/v1/models`) · `openai-model-get` (GET `/v1/models/{model}`) ·
      `openai-chat-completion` (POST `/v1/chat/completions`) ·
      `openai-embeddings-create` (POST `/v1/embeddings`).
- [ ] **No operation declares a string-ish or `Any`-typed query parameter.** C-30 is not implemented,
      so the emitter still emits such values unencoded — `zendesk-ticket-search` is the standing proof
      that this corrupts requests and can inject parameters. Any operation that would need one is left
      out and named in Notes instead. A test asserts the connector declares no query parameters at all.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/openai.flux` and
      `connectors/openai.connector.toml`, both committed and reviewed, and a second build is
      byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point tests every
      existing provider passes.
- [ ] `crates/catalog/src/generated.rs` gains its `pub(crate) mod openai;` line and its `PROVIDERS`
      entry in id order — `crates/catalog/tests/embedded_operations.rs` fails until it does.
- [ ] `http_hosts` in the manifest is `api.openai.com`, never widened; no credential value appears in
      any generated artifact.
- [ ] The cost-bearing nature of `chat-completion` and `embeddings-create` is reflected in `risk` —
      an operation an LLM can call that spends money is not `low`.

## Progress
- Not started. Filed 2026-07-30 under "ship up to 3 connectors, popular and useful".

## Notes
- **Still cannot make a live call**, for the same reason as every other connector: flux's
  `http.request` takes `{"$secret": ...}` as a whole-value replacement, so no `Bearer ` prefix is
  produced (`docs/designs/auth-seam.md`). This connector is the catalogue's proof and the pipeline's,
  not a working client yet.
- Deliberately excluded pending C-30 and flux's structured query map: the `limit`/`after` listing
  parameters on `/v1/models` and the assistants surface.
- **Anthropic is the obvious fourth**, also named in `AGENTS.md`'s charter: `x-api-key` is
  `scheme = "header"` and the body shape mirrors chat completions, so it costs one more TOML once
  this one's shape is settled.
