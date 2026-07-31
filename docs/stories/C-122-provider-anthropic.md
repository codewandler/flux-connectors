---
id: C-122
title: "Ship the Anthropic connector — management surface and model catalogue"
pillar: Spec
status: in-progress
priority: 4
design: docs/designs/provider-roles.md
epic: provider-roles
areas: [providers]
note: "the third vendor to fill llm_catalogue, and the one that proves the role is vendor-independent rather than shaped around OpenAI. Management plane only — inference stays with flux's native anthropic provider"
---

# Ship the Anthropic connector — management surface and model catalogue

## Goal

Add `providers/anthropic.toml` covering Anthropic's **management** surface — model listing, and the
Admin API operations worth exposing — and have it claim `llm_catalogue`.

## Acceptance

- [x] `providers/anthropic.toml` ships with authority `com.anthropic`, **and, deliberately, a
      `header` (`x-api-key`) credential rather than the `bearer` this line names** — Anthropic's own
      API authenticates with `x-api-key: <secret>`, never `Authorization: Bearer`, and shipping a
      literally-`bearer` credential would send every request without a working key. See the
      Progress note. Operations are selected rather than mechanically enumerated: 5 of Anthropic's
      much larger surface, read-only, chosen for what a management flow actually needs.
- [x] A `models` service claims `llm_catalogue` with `list` and `get`
      (`anthropic-models-list`, `anthropic-model-get`), satisfying
      [C-121](C-121-llm-catalogue-role.md)'s contract **without any change to the role's
      definition** — `crates/connector-flux/tests/anthropic_connector.rs`'s
      `the_models_service_claims_llm_catalogue_without_reshaping` asserts it directly against
      `Connector::roles()` and `missing_role_members`. The role was not shaped around OpenAI: an
      `x-api-key`-authenticated, dated-header-versioned vendor with an entirely separate Admin
      surface still fills the same one-slot contract unchanged.
- [ ] **Not reachable from this story.** "The connector appears in the model pool alongside `openai`
      and `openrouter`" needs two things neither of which exists yet in `main`: a model-pool
      projection (C-121's Acceptance, not built) and a `[[services]]` `roles = ["llm_catalogue"]`
      declaration on `openai` and `openrouter` themselves (also C-121's — `providers/openai.toml`
      and `providers/openrouter.toml` are outside this story's fence). This connector is forward
      -compatible with both: it already claims the role through the existing C-120 mechanism, so the
      day C-121 lands the pool and the two sibling declarations, anthropic appears in it with no
      further change here. Filed as a dependency gap rather than worked around.
- [x] **No inference operation.** No `/messages` or `/complete` path anywhere in the file;
      `no_operation_is_the_messages_inference_endpoint` asserts it, and every operation is a `GET`.
- [x] Generated Flux parses, analyzes, and is a fixed point of flux's own formatter —
      `every_anthropic_operation_emits_an_analyzable_module`, plus
      `cargo run -p connector-cli -- build --provider anthropic` / `diff --provider anthropic`
      report no drift.
- [x] No credential value anywhere shipped, and neither `[[config]]` field carries an `example` —
      `the_credentials_are_configurable_and_carry_no_example_value`, plus the workspace-wide
      `no_provider_file_carries_a_credential_value`.
- [x] The build stays a fixed point (`10 artifacts up to date (1 provider checked)`) and the scoped
      gate is green; the whole-workspace gate is green modulo the eight whole-catalogue staleness
      tests AGENTS.md tabulates as the expected result of adding a provider.

## Notes

- **Depends on C-120 and C-121.** Without the role mechanism this is just an eighteenth connector.
- The point of this story is *falsification*: two providers fitting a role proves little when both
  were used to design it. Anthropic's management API is shaped differently enough to test whether
  `llm_catalogue` describes a capability or just describes OpenAI.
- Anthropic's Admin API (workspaces, members, API keys) is a genuine management surface and a
  reasonable second service — but scope it by what is actually useful, and leave anything unclear out
  rather than guessing at semantics.
- This is a **paid SaaS vendor's control plane**, which is squarely what this repo is for. The
  non-goal is only the inference path.

## Progress

Landed on `impl/C-122`. `providers/anthropic.toml` ships 5 read-only GET operations across two
services — `models` (`anthropic-models-list`, `anthropic-model-get`, claiming `llm_catalogue`) and
`admin` (`anthropic-organization-get`, `anthropic-workspaces-list`, `anthropic-api-keys-list`).
`crates/connector-flux/tests/anthropic_connector.rs` is the contract suite, mirroring
`notion_connector.rs`'s shape for the const-header assertion this story is about.

Verified against Anthropic's own published API reference (models list/get, Admin API overview,
`organizations/me`, `workspaces` list, `api_keys` list) rather than from memory, since AGENTS.md
refuses an invented endpoint. Two corrections against the story text itself, both recorded rather
than silently applied:

1. **`bearer` → `header` (`x-api-key`).** Anthropic's real wire auth is a custom header, never
   `Authorization: Bearer`. Shipping the literal scheme the story names would produce a connector
   that sends every request without a credential that resolves to anything — a `bearer` connector
   for an `x-api-key` vendor. `crates/connector-spec/src/auth.rs`'s `AuthScheme::Header` variant
   covers it (`scheme = { header = { name = "x-api-key" } }`), the same mechanism already shipped for
   babelforce's excluded pair and documented in that file's auth module docs.

2. **Two credentials, not one.** The Admin API accepts only an Admin API key
   (`sk-ant-admin…`) or an `org:admin` OAuth token — never the regular API key the Models API takes.
   Both are `x-api-key`-shaped, so it would have been easy to fold them into one declared credential,
   but that would ask every operator for organization-admin access merely to list models. `[[auth]]`
   declares `anthropic.api_key` (default, used by the two `models` operations and by `verify`) and
   `anthropic.admin_key` (named explicitly via `auth = [...]` on each of the three `admin`
   operations), each with its own `[[config]]` field and its own `help` text explaining which
   operations need it.

**The model-pool acceptance item is not satisfiable from this story alone**, and is left unchecked
above rather than worked around: the pool projection and the `openai`/`openrouter` role declarations
are C-121's Acceptance items, not landed in `main` as of this branch's merge-base, and both files are
outside this story's fence (`providers/openai.toml`, `providers/openrouter.toml`, and any
`connector-catalog`/`connector-spec` pool code belong to whichever story builds the pool). What this
story *can* do — claim the role correctly on its own service, so no further edit to
`providers/anthropic.toml` is needed once the pool exists — is done and tested
(`the_models_service_claims_llm_catalogue_without_reshaping`).

Every operation is a `GET`; the connector has no body parameter and therefore never exercises
`BodyNode`'s array limitation (C-168/C-185) or the POST/PATCH idempotency refusal (C-186) — both
were considerations while selecting the surface, not obstacles this file had to route around, since
the Admin API's mutating operations (invites, member-role updates, workspace archiving, API-key
status changes) were left out on their own merits (semantics unclear enough to guess at) rather than
because either gap forced the exclusion.

`cargo run -p connector-cli -- build --provider anthropic` then `diff --provider anthropic` report
`10 artifacts up to date (1 provider checked)` with no drift. The full workspace gate
(`cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`) is green except
for the eight whole-catalogue staleness tests AGENTS.md's table predicts for any new-provider story;
the ninth, ratchet-owned `the_recorded_floor_is_the_measured_figure`, stayed green in this worktree.
