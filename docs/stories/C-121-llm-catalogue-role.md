---
id: C-121
title: "The llm_catalogue and ticketing roles, and the model pool they feed"
pillar: Spec
status: ready
priority: 3
design: docs/designs/provider-roles.md
epic: provider-roles
areas: [connector-spec, codegen, providers]
note: "two roles, not one — a mechanism validated by a single role is designed around a single case. Connectors contribute a LIVE model list; flux's static pricing tables are explicitly 'the fallback'"
---

# The llm_catalogue and ticketing roles, and the model pool they feed

## Goal

Define the first two roles, assign them to the providers that already fit, and project the
`llm_catalogue` members into a **model pool** a host can read.

## Acceptance

- [ ] Two roles are defined against the closed set from [C-120](C-120-service-roles-declaration.md):

      | role | required members | providers that must claim it |
      |---|---|---|
      | `llm_catalogue` | `list`, optional `get` | `openai`, `openrouter` |
      | `ticketing` | `show`, `search`, `comment.list` | `zendesk`, `freshdesk`, `intercom`, `jira` |

- [ ] Assigning a role to those providers requires **no change to any operation** — if a provider has
      to be reshaped to fit, either the role's contract is wrong or that provider does not have the
      capability. Say which, in the story's Progress, rather than bending the provider.
- [ ] Roles reach the manifest and `catalog.json`, and `catalog` gains a way to ask which providers
      hold a role.
- [ ] A **model pool** projection: for every service holding `llm_catalogue`, expose the operation
      that enumerates models, so a host can build `(provider, model)` tuples. The pool carries model
      **ids** only — context windows and pricing are deliberately out of scope here.
- [ ] **Failing-first test:** `every_declared_role_is_satisfied_by_its_provider` — iterate the shipped
      catalogue, and for each declared role assert the required members exist with a satisfying shape.
      It must fail if a role is assigned to a provider that does not implement it.
- [ ] A test asserts the pool is **non-empty** and names at least two distinct providers, so the
      projection cannot pass vacuously.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **`ticketing` is not filler.** A role mechanism validated by exactly one role is designed around one
  case. Four shipped providers already fit this shape, and flux's retained
  `examples/zendesk.triage.flux` is written against precisely `show` / `search` / `comment.list` — so
  the role has a real consumer immediately.
- **The pool is informed here, served by flux.** `ai.extract|judge|rank|reason|rewrite` keep resolving
  `(provider, model)` through `flux-providers`' own spec resolution, which
  `crates/flux-providers/src/spec.rs` centralises so a spec resolves identically everywhere. Nothing
  in this story routes an inference call.
- The value this adds over flux's status quo is *liveness*: flux's model metadata lives in static
  tables per provider module, and the pricing table is explicitly the fallback.
  `openai-models-list` is a live call.
- Expect vendor disagreement about what a "model id" is (`gpt-4o` vs `openai/gpt-4o` on OpenRouter's
  proxied slugs). Record the rule you pick; do not normalise silently.
