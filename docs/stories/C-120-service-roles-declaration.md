---
id: C-120
title: "Declare roles on a service, with the closed set and its refusals"
pillar: Spec
status: ready
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

- [ ] `[[services]]` accepts `roles = ["..."]`. `Service` gains the field in
      `crates/connector-spec/src/ir.rs`, serialized with `skip_serializing_if` so a provider that
      declares none hashes exactly as before.
- [ ] `Connector::roles()` derives the provider's set as the union of its services'. **Never
      authored** — there is no provider-level `roles` key, and one in a TOML file is a load error.
- [ ] A `Role` is a closed enum with a required-member contract, not a free string.
- [ ] **Refusals, each with its own test:**
      - an unknown role name → refused, naming the role and listing the known set;
      - a service claiming a role without the role's required members → refused, naming the missing
        member;
      - a provider-level `roles` key → refused, pointing at the service level;
      - a duplicate role on one service → refused.
- [ ] The reserved `default` service can carry roles, since a single-surface provider has nowhere else
      to put them.
- [ ] **Failing-first test:** `a_service_claiming_a_role_it_does_not_satisfy_is_refused` — must fail
      before the check exists.
- [ ] The gate is green, and `cargo run -p connector-cli -- build` still reports a fixed point.

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
