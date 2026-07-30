---
id: C-120
title: "Declare roles on a service, with the closed set and its refusals"
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/provider-roles.md
epic: provider-roles
areas: [connector-spec]
note: "the mechanism — roles attach to a SERVICE and a provider's are derived; an unknown role name is a load error, because a typo'd capability that silently means 'no capability' is the whole failure mode"
---

# Declare roles on a service, with the closed set and its refusals

## Goal

Add `roles` to a service in the IR and the loader, as a **closed, checkable** set. This story is the
mechanism only — the two concrete roles land in C-121 and are used there.

## Acceptance

- [x] `[[services]]` accepts `roles = ["..."]`. `Service` gains the field in
      `crates/connector-spec/src/ir.rs`, serialized with `skip_serializing_if` so a provider that
      declares none hashes exactly as before.
- [x] `Connector::roles()` derives the provider's set as the union of its services'. **Never
      authored** — there is no provider-level `roles` key, and one in a TOML file is a load error.
- [x] A `Role` is a closed enum with a required-member contract, not a free string.
- [x] **Refusals, each with its own test:**
      - an unknown role name → refused, naming the role and listing the known set;
      - a service claiming a role without the role's required members → refused, naming the missing
        member;
      - a provider-level `roles` key → refused, pointing at the service level;
      - a duplicate role on one service → refused.
- [x] The reserved `default` service can carry roles, since a single-surface provider has nowhere else
      to put them.
- [x] **Failing-first test:** `a_service_claiming_a_role_it_does_not_satisfy_is_refused` — must fail
      before the check exists.
- [x] The gate is green, and `cargo run -p connector-cli -- build` still reports a fixed point.

## Notes

- Roles attach to a **service** because `openai`'s model-listing surface and its chat surface are
  different capabilities of one vendor, and C-49's service level already models that. A role on the
  provider would smear them together.
- Derivation follows the precedent in
  [connector-configuration.md](../designs/connector-configuration.md): `Level` is derived from
  `binds` and never written by hand. Do the same here rather than inventing a second convention.
- Required members are named by their **member name within the service** (`list`, `show`), not by the
  full operation id — that is what makes the shape vendor-independent.
- Do not ship the roles' *definitions* here beyond what the tests need; C-121 owns `llm_catalogue` and
  `ticketing` and their assignment to shipped providers. Keep this story to the mechanism so its
  refusals are reviewable on their own.
- No shipped provider TOML needs to change in this story. If you find yourself editing
  `providers/*.toml`, you are doing C-121's work.

## Progress

Landed on `impl/C-120`. `crates/connector-spec/tests/service_roles.rs` is the suite; the four
refusals also have golden snapshots under `tests/golden/`.

Three decisions a follow-up should know about, because each one resolved something the story left
open:

1. **Only `llm_catalogue` is defined.** The story says not to ship role definitions beyond what the
   tests need, but a closed enum with no variants is not a checkable mechanism, so exactly one
   variant exists — `Role::LlmCatalogue`, requiring a `list` member. **C-121 adds `ticketing`**
   (`show`, `search`, `comment.list`) and assigns both to shipped providers. Adding a variant means
   `Role::ALL`, `Role::word` and `Role::required_members` in `ir.rs`, plus the `role` enum in
   `schema/provider-toml.schema.json`.

2. **"Member name within the service" is implemented as the member's trailing name segments** —
   `ir::fills_slot`. A role requires `list`; `openai-models-list` and `openrouter-models-list` both
   fill that slot, and the vendor prefix nobody agreed on stays out of the contract. Segments split
   on `-`, `_` and `.`, so the design's `comment.list` and `zendesk-ticket-comment-list` are the same
   slot. Matching is on whole segments, never on a substring, so `acme-models-listing` does not fill
   `list`. This was the one genuinely underdetermined point in the story: an exact match against
   `Connector::member_names_of` would have been the other reading, but a member name for an operation
   *is* its full id, and the story ruled that out explicitly.

3. **A `[[services]]` entry may name the reserved `default`, but only to carry `roles`.** C-49
   refused the name outright, because a second definition of the implicit service could disagree with
   the connector about a base URL or a version. `roles` has no connector-level spelling, so it has
   nothing to contradict — and a single-surface provider has no other service to attach a role to.
   The entry is refused if it also states `description`, `base_url` or `api_version`, and refused if
   it states no roles at all. `Connector::is_default_only` was widened accordingly ("no service other
   than `default` is declared"), so writing the entry does **not** rename a provider's artifacts.
   `tests/golden/reserved-default-service.*` was updated for the narrowed message.

No `providers/*.toml` changed and no artifact was regenerated: `cargo run -p connector-cli -- build`
still reports `17 providers, 236 artifacts up to date; nothing written`.
