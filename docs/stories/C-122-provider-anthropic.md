---
id: C-122
title: "Ship the Anthropic connector — management surface and model catalogue"
pillar: Spec
status: ready
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

- [ ] `providers/anthropic.toml` ships with authority `com.anthropic`, bearer credential, and its
      operations selected rather than mechanically enumerated (the standing rule: a connector selects
      the operations worth exposing).
- [ ] A `models` service claims `llm_catalogue` with `list` and `get`, satisfying
      [C-121](C-121-llm-catalogue-role.md)'s contract **without any change to the role's definition**.
      If the role has to move to accommodate Anthropic, the role was shaped around OpenAI — say so.
- [ ] The connector appears in the model pool alongside `openai` and `openrouter`.
- [ ] **No inference operation.** `POST /v1/messages` is not in this connector. flux's native
      `anthropic` provider owns that path, and duplicating it here is the non-goal C-119 records.
- [ ] Generated Flux parses, analyzes, and is a fixed point of flux's own formatter — the standing
      per-provider gate every other connector already passes.
- [ ] No credential value in the TOML, the generated module, the manifest, the catalogue or the
      lockfile. **A `secret` field must not carry a realistic-looking `example`** — a placeholder
      shaped like a real token trips GitHub's push protection and has blocked a release here before.
- [ ] The build stays a fixed point and the full gate is green.

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
