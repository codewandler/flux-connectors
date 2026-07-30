---
id: C-76
title: Ship the OpenRouter connector
pillar: Spec
status: done
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · OpenAI-compatible · charter-named
---

# Ship the OpenRouter connector

## Goal
Ship the third model provider `AGENTS.md` names in its charter, and the cheapest connector in
the fleet: OpenRouter speaks the OpenAI request shape, so C-51's operations transfer almost unchanged.

## Acceptance
- [x] `providers/openrouter.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [x] `base_url = "https://openrouter.ai"`, `vendor = "OpenRouter"`, and a `[[auth]]` entry with `scheme = "bearer"` over `OPENROUTER_API_KEY`, named by `default_auth`.
- [x] A curated set of roughly three over `/api/v1`: chat completion, models list, generation get
      — the OpenAI-compatible subset. **Four ship, and generation get is not among them** — its id is a
      required query parameter, so it is unemittable rather than merely awkward. See Notes.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [x] `cargo run -p connector-cli -- build` emits `connectors/openrouter.flux` and
      `connectors/openrouter.connector.toml`, both committed, and a second build is byte-identical.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [x] **`max_tokens` is required, as C-51 made `max_completion_tokens` required**: an operation an LLM
      can call that spends money must not be unbounded, and required also sidesteps the optional-body
      `null` gap (C-56). **Satisfied as `max_completion_tokens`, not `max_tokens`** — the vendor's own
      document marks `max_tokens` deprecated. See Notes.
- [x] The optional `HTTP-Referer` and `X-Title` attribution headers are **not** declared as caller
      parameters; they are constant headers, which is C-55's subject. Record the omission.
- [x] The story records how much of `providers/openai.toml` transfers and what differs, so the next
      OpenAI-compatible vendor is a copy rather than a rediscovery.

## Progress
- **Done.** Four operations over `/api/v1`, one bearer credential, zero query parameters, zero optional
  body fields. `cargo run -p connector-cli -- build` writes 8 new artifacts and `diff` reports
  `107 artifacts up to date (12 providers checked)`.
- Two hand-maintained files were edited: `crates/catalog/src/generated.rs` (the `mod` line and the
  `PROVIDERS` entry, in id order) and `operation_selection_stays_curated` in
  `crates/connector-spec/tests/shipped_providers.rs` (`("openrouter", 4)`). The provider *set* is
  derived from `providers/` everywhere else (C-54), so nothing else needed a new entry.
- New tests: `crates/connector-flux/tests/openrouter_connector.rs` (9 tests, the connector's own
  contract) and `openrouter_publishes_one_host_and_no_credential_anywhere` in
  `crates/connector-cli/tests/shipped_providers_build.rs`, which is the acceptance item about the
  manifest and the public catalogue — it carries the check one artifact further than the intercom
  equivalent, into `web/public/catalog.json`.
- **The operation set was authored against the vendor's own OpenAPI document**, fetched from
  `https://openrouter.ai/openapi.json` (~1.6 MB, `info.version` 1.0.0) and read directly rather than
  from documentation prose. Nothing was vendored under `specs/` — C-4's ingest and C-14's fetch do not
  exist — but every path, method, parameter location and `required` list below was checked against it.
  That document is what turned two of the story's assumptions into corrections; see Notes.

## The transfer from `providers/openai.toml`

The acceptance item, recorded here as well as in the provider file's §1 so it survives a reader who
never opens the TOML.

**Transferred unchanged** — roughly the whole file's *reasoning*, and most of its structure:

- The entire auth model: one bearer credential, one env var, no user half, no `[auth.oauth2]`, declared
  once in `default_auth` and inherited. OpenRouter's document declares `apiKey` (`type: http`,
  `scheme: bearer`) once at the root and no operation overrides it — the same shape OpenAI's does.
- The selection principle (JSON in, JSON out, no query string), the body-field discipline (required
  only), and the risk/idempotency argument for a per-token-billed `POST` (`medium`,
  `non_idempotent`) — verbatim, including why `medium` rather than `high`.
- `messages` as `array` of `object` → `List<Any>`, and the honesty argument for it.
- The `max_completion_tokens` **field name and its required-ness**, which is the outcome the story did
  not expect; see below.
- The path-parameter safety argument and its shape: a path value is interpolated as verbatim as a query
  value is, and is safe only when the charset is.

**Differed** — five places a copy-paste would have been wrong:

1. `base_url` is the bare site and every `path` carries `/api/v1`, not OpenAI's `/v1`. The vendor's
   `servers[0].url` is `https://openrouter.ai/api/v1`; this story's Acceptance fixes the shorter
   `base_url`, so the prefix moved into each path.
2. The token-budget field name — see Notes. The story predicted a difference here and there is none;
   the difference is that the *story's* name was wrong.
3. `model` is optional at OpenRouter (it falls back to the account's dashboard default) and required
   here. OpenAI requires it outright.
4. There is no `openrouter-model-get`. OpenRouter has no `/models/{model}` read; the nearest is
   `/api/v1/models/{author}/{slug}/endpoints`, which splits a model id at its `/` into two path
   parameters — and that split is why it is emittable at all, since the one character in a model id
   that could reshape a URL is never inside either half.
5. The verification operation is `openrouter-credits-get`, not the models list. OpenAI's
   `GET /v1/models` doubles as its key check; OpenRouter's models list is a public catalogue
   documenting 400/403/500 and no 401, while `/api/v1/credits` documents 401 and 403.

**Available and verified but not selected:** `POST /api/v1/embeddings`, whose required body is
`["input", "model"]` — a direct transfer of `openai-embeddings-create` down to the scalar-or-array
`input` union. It needs no new capability from this repository. It is left out only because C-76 asks
for roughly three operations and already carries four.

## Notes
- Cost-bearing operations carry a `risk` above `low`, following C-51.
- **`GET /api/v1/generation` is excluded, and this replaced rather than trimmed the story's set.** The
  Acceptance names it as one of roughly three operations to ship. Its generation id is
  `{"name": "id", "in": "query", "required": true}` in the vendor's document, and it is the endpoint's
  *only* input — there is no path-shaped spelling to fall back on, so under C-30 the operation is
  unemittable rather than awkward. `openrouter-credits-get` ships in its place as the account-level
  substitute: it reads the balance the generation endpoint would report per request. Per-request cost
  accounting returns when C-30 lands.
- Deliberately excluded pending C-30: **all 29** of the models list's optional query filters —
  `offset`, `limit`, `category`, `supported_parameters`, `output_modalities`, `input_modalities`,
  `sort`, `q`, `context`, the price/age/index bounds and the rest. All are optional, so the operation
  is well-formed with none; a caller receives the whole catalogue and filters it itself. `q` is the
  most tempting and the worst — a free-text search value is exactly `zendesk-ticket-search`'s shape.
- Deliberately excluded pending C-56, every one of them optional in the vendor's `ChatRequest`:
  `temperature`, `top_p`, `top_k`, `top_a`, `min_p`, `seed`, `stop`, `frequency_penalty`,
  `presence_penalty`, `repetition_penalty`, `logit_bias`, `logprobs`, `top_logprobs`,
  `response_format`, `tools`, `tool_choice`, `parallel_tool_calls`, `reasoning`, `reasoning_effort`,
  `prediction`, `modalities`, `user`, `metadata`, `prompt_cache_key` — and, most costly for this
  vendor specifically, **`models`** (the fallback list) and **`provider`** (routing preferences). Those
  last two are the features a caller reaches for OpenRouter *for*, and both are optional objects, so
  both wait for C-56. OpenRouter makes the gap worse than it is for a first-party vendor: it does not
  execute the request, it forwards it to whichever upstream serves the model, so a forwarded
  `{"temperature": null}` is evaluated against a few hundred schemas OpenRouter does not control.
  `stream` is excluded for the unrelated and permanent-looking reason that `http.request` returns one
  flat string.
- **DEVIATION — the token budget is `max_completion_tokens`, not the `max_tokens` this story's
  Acceptance names.** The Acceptance's *reasoning* is implemented exactly: the field is required, so an
  operation an LLM can call cannot be unbounded and the C-56 null is sidestepped by construction. Only
  the name changed, and the vendor is the reason. Its own document describes the two fields as
  `"max_tokens": "Maximum tokens (deprecated, use max_completion_tokens). Note: some providers enforce
  a minimum of 16."` and `"max_completion_tokens": "Maximum tokens in completion"`. So the story's
  premise — that OpenRouter's OpenAI-compatible surface is the *legacy* shape and therefore wants the
  legacy name — does not hold, and authoring against `max_tokens` would repeat the exact mistake
  `providers/openai.toml` records avoiding. `the_chat_completion_requires_a_non_deprecated_token_budget`
  asserts the deprecated spelling is **absent**, not merely unused, so an author working from the story
  text alone fails a test that explains why.
- **`model` is also required though the vendor marks it optional** — a second narrowing the story does
  not ask for. A request naming no model is routed to whatever default the account configured in its
  dashboard, so the op's cost, latency and capabilities would be set by out-of-band configuration this
  repository cannot see, and two callers of the same op id would get different models.
- **The attribution headers are omitted, not declared.** `HTTP-Referer` and the title header — spelled
  `X-Title` in older documentation and `X-OpenRouter-Title` in current — place an app on OpenRouter's
  public leaderboards, and the chat completion additionally accepts an optional
  `X-OpenRouter-Metadata`. All are optional, so every operation is well-formed without them. None is
  declared as a `params.header` entry: a `const`-pinned header emits as a required, caller-overridable
  argument with the `const` dropped (C-52's finding), which is a disguise rather than a constant.
  Pinning them is [C-55](C-55-constant-request-headers.md); the cost until then is leaderboard
  attribution only, and `no_openrouter_operation_declares_a_header_parameter` keeps the disguise out.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
- OpenRouter publishes an OAuth2 PKCE flow (`/api/v1/auth/keys`) for apps that mint a key on a user's
  behalf. That is effectful acquisition — C-21's, and the host's — so it is deliberately not modelled.
- The vendor's document carries **68 paths**, including workspace administration, budgets, organization
  members, API-key minting, guardrails, files, and video and image generation. That is the concrete
  argument for an opt-in `[[patch.operations]]` selection when C-4 lands rather than a whole-document
  ingest: 68-plus operations would be 68-plus LLM tools, most of them destructive account admin.
