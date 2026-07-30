---
id: C-51
title: Ship the OpenAI connector
pillar: Spec
status: in-progress
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
- [x] `providers/openai.toml` is hand-authored — OpenAI publishes an OpenAPI document but C-4's
      ingest is not implemented, so this follows the zendesk precedent (`[spec]` pointer noted as the
      later form, operation set as the selection to reproduce).
      → `providers/openai.toml:1-20` header. Unlike zendesk, the header records that a *usable* spec
      exists (`github.com/openai/openai-openapi`) and that only missing ingest keeps this inline.
- [x] `base_url = "https://api.openai.com"`, `vendor = "OpenAI"`, and a `[[auth]]` entry with
      `scheme = "bearer"` over `OPENAI_API_KEY`. `default_auth` names it for every operation.
      → `providers/openai.toml:22-50`; asserted by
      `openai_connector.rs::every_operation_authenticates_as_bearer_over_one_env_var`.
- [x] A curated operation set of roughly four, each with `risk` and `idempotency`. Confirm the
      shapes against current vendor docs before authoring; the intended set is
      `openai-models-list` (GET `/v1/models`) · `openai-model-get` (GET `/v1/models/{model}`) ·
      `openai-chat-completion` (POST `/v1/chat/completions`) ·
      `openai-embeddings-create` (POST `/v1/embeddings`).
      → all four, exactly as named. Paths, methods, the `model` path parameter, the absence of query
      parameters on all four, and bearer security were checked against OpenAI's published
      `openapi.yaml`, not authored from memory.
- [x] **No operation declares a string-ish or `Any`-typed query parameter.** C-30 is not implemented,
      so the emitter still emits such values unencoded — `zendesk-ticket-search` is the standing proof
      that this corrupts requests and can inject parameters. Any operation that would need one is left
      out and named in Notes instead. A test asserts the connector declares no query parameters at all.
      → zero query parameters of any type;
      `openai_connector.rs::the_openai_connector_declares_no_query_parameter_at_all`. Confirmed
      end-to-end in `web/public/catalog.json`: openai is the only provider with no
      `unencodable-query-value` issue anywhere.
- [x] `cargo run -p connector-cli -- build` emits `connectors/openai.flux` and
      `connectors/openai.connector.toml`, both committed and reviewed, and a second build is
      byte-identical.
      → first run wrote 8 artifacts; second run reported `44 artifacts up to date; nothing written`,
      and `diff` reports `44 artifacts up to date (4 providers checked)`.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point tests every
      existing provider passes.
      → `openai` added to the five `SHIPPED` lists, plus a per-operation gate in
      `openai_connector.rs::every_openai_operation_emits_an_analyzable_module`.
- [x] `crates/catalog/src/generated.rs` gains its `pub(crate) mod openai;` line and its `PROVIDERS`
      entry in id order — `crates/catalog/tests/embedded_operations.rs` fails until it does.
      → `crates/catalog/src/generated.rs:15` and `:25`, both between `freshdesk` and `zendesk`.
- [~] `http_hosts` in the manifest is `api.openai.com`, never widened; no credential value appears in
      any generated artifact.
      → **the manifest has no `http_hosts` field yet** — `connector.connector.toml` is a placeholder
      shape and the allowlist lands in C-10 (`crates/connector-cli/src/seam.rs:224-261`, and the
      generated file's own header says so). The *fact* is committed everywhere it currently can be:
      `hosts: &["api.openai.com"]` in `crates/catalog/src/generated/openai.rs`, and
      `every_request_targets_api_openai_com_and_nothing_wider` pins it against the emitted `$base`
      and refuses a templated `base_url`. No credential value appears in any artifact
      (`the_emitted_flux_carries_no_credential_at_all`); `OPENAI_API_KEY` appears once, as an env-var
      *name*, in `web/public/catalog.json`'s credential declaration, which AGENTS.md permits.
- [x] The cost-bearing nature of `chat-completion` and `embeddings-create` is reflected in `risk` —
      an operation an LLM can call that spends money is not `low`.
      → both `risk = "medium"`, `idempotency = "non_idempotent"`, held by
      `openai_connector.rs::the_cost_bearing_operations_declare_what_they_cost`. `chat-completion`
      additionally makes `max_completion_tokens` **required**, so no call is unbounded in cost.

## Progress
- **Done bar one item, which is blocked on C-10 rather than on this story.** Four operations, four
  generated artifacts, 29 catalogued operations across 4 providers.
- `http_hosts` could not be written because the manifest has no such field yet — see the `[~]` item.
  When C-10 adds it, `api.openai.com` is the value, and it must not be widened to `*.openai.com`.
- **The one finding a resuming agent should act on:** the emitter cannot omit an optional *body*
  field. Query parameters get a `when $x` guard so null means "not sent"; body fields are placed into
  the payload unconditionally, so a `required = false` body field the caller omits travels as an
  explicit `{"field": null}`. OpenAI documents its inference knobs as *omitted* when unset, not as
  nullable, so every optional tuning parameter (`temperature`, `top_p`, `n`, `stop`, `seed`,
  `response_format`, `tools`, and embeddings' `dimensions`/`encoding_format`) is deliberately left out
  rather than shipped as a probable-null. Closing that gap in `connector-flux` — a `when`-guarded body
  field, mirroring what query parameters already have — unblocks all of them at once and is worth its
  own story. Recorded in full as a `SCHEMA GAP` in `providers/openai.toml`.
- `AGENTS.md` and `README.md` still say "three providers · 25 operations · 37 artifacts". Both are
  now stale (4 · 29 · 44) and are left to the coordinator, since C-52 and C-53 move the same numbers.

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
