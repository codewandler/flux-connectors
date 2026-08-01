---
id: C-441
title: "Widen the Anthropic Admin service to the members, workspace and invite reads it does not have"
pillar: Spec
status: ready
priority: 2
epic: provider-roles
areas: [providers]
note: "the admin service ships three reads and the Admin API has more; every one is an unparameterized GET because C-30 leaves no query encodable, and most of them carry personal data — which is the part to get right"
---

# Widen the Anthropic Admin service to the members, workspace and invite reads it does not have

## Goal

`providers/anthropic.toml`'s `admin` service ships **three** operations —
`anthropic-organization-get`, `anthropic-workspaces-list`, `anthropic-api-keys-list`. The Admin API
offers more reads that a flow genuinely wants: who is in the organization, who is in a workspace,
one workspace by id, and which invites are outstanding. Add them, to the same evidence standard the
existing three were held to.

## Scope: reads only, and the reason is already recorded

`providers/anthropic.toml:99-104` states why the mutating surface was left out of C-122, and that
reasoning stands unchanged:

> The Admin API's mutating surface (creating invites, updating member roles, archiving workspaces,
> rotating API-key status) is real and useful, but each one needs a decision about request/response
> shape this story does not need to make to satisfy its Goal, so it is left out rather than guessed
> at — the standing rule this repository applies everywhere a shape is unclear.

**Do not add a POST, PATCH or DELETE in this story.** One of the two constraints that note cites has
since closed and one has not — [C-186](C-186-idempotent-post-patch.md) is `done`, but
[C-185](C-185-body-arrays.md) (a request body cannot contain an array) is still `ready`. That does
not matter here, because reads have no body at all; it is recorded so nobody re-derives it.

## Every operation is unparameterized, and that is not an oversight

The three shipped operations declare **no parameters**, and the file says why at `:84-85`, `:249`,
`:335` and `:358`: [C-30](C-30-refuse-unencodable-query.md) is `ready`, so no query parameter is
encodable and the emitter refuses the ones that are not. Every list added here inherits that. Follow
the shipped convention exactly:

- declare the cursor fields (`first_id`, `last_id`, `has_more`) in the **response** schema, each with
  a description saying plainly that this connector cannot feed them back as `after_id`/`before_id`;
- state in the operation `description` that the call is unpaginated, so a caller is not misled into
  believing it sees the whole organization.

A **path** parameter is fine and is how `anthropic-model-get` already works — that is the mechanism
for `workspace_id` and `user_id`.

## The part to get right: these operations return personal data

Organization members, workspace members and invites carry **names and email addresses of real
people**. This repository already has a convention for that and it is not decorative — see
`providers/docusign.toml:299-301`, `providers/bitbucket.toml:428` and `providers/jira.toml:362`,
which each mark such a field in the description a *model* reads:

> Identifies a named person — read it for what the calling flow needs and do not persist it beyond
> that

Every field that names or contacts a person carries that sentence. Do not add a field whose only
purpose is to surface an email address more conveniently.

## Acceptance

- [ ] The `admin` service gains the Admin API's remaining **read** operations: organization members
      (list, and get by id), workspace members (list for a workspace, and get by id), one workspace
      by id, and outstanding invites (list). Name them in the shipped style —
      `anthropic-<thing>-<verb>`, unique across the service's one member namespace.
- [ ] Every added operation is a `GET`, declares `auth = [{ credentials = ["anthropic.admin_key"] }]`,
      `risk = "low"`, `idempotency = "idempotent"`, and an `[operations.quirks.error_envelope]` with
      the same two pointers the shipped three use.
- [ ] Every list declares `first_id`, `last_id` and `has_more` in its response schema, each
      documented as unusable for paging here, and says in its `description` that it is unpaginated.
- [ ] **Every field naming or contacting a person carries the personal-data sentence** the three
      cited providers use. A test or a grep in the story's Progress shows it holds for each.
- [ ] **Nothing is invented.** `specs/anthropic/2023-06-01-excerpt.yaml` is an excerpt and does not
      describe the Admin API, so these are hand-authored like the shipped three. Any field not known
      with confidence is left out or left untyped with a note saying so — never guessed into a
      `required` list. This is [C-126](C-126-response-schema-coverage.md)'s rule and it is the one
      most likely to be broken here.
- [ ] No POST, PATCH or DELETE is added.
- [ ] `cargo run -p connector-cli -- build` succeeds and `-- diff` shows **only** the intended
      additions — no other provider's artifacts move.
- [ ] The workspace gate is green: `cargo test --workspace`, `cargo fmt --all --check`, and whatever
      else `AGENTS.md` names.
- [ ] **Failing-first:** a test that fails before the change and passes after. `babelforce_coverage.rs`
      is the shape to copy — assert the `admin` service's exposed operation set is exactly the
      expected list, so the count and the names are both pinned.

## Progress
- (not started)

## Notes
- The service and its credential already exist (`providers/anthropic.toml:226-228`,
  `anthropic.admin_key`); this story adds operations to them and declares no new service and no new
  credential.
- `anthropic-api-keys-list`'s response schema at `:366` is the best model to copy for shape and for
  tone — note how it documents `partial_key_hint` as *"a partially redacted display string, safe to
  show in a UI — never the key"*. That is the level of care expected.
- Roles are unaffected: `llm_catalogue` sits on the `models` service, not on `admin`.
- Do not touch the `[[services]]` blocks. C-153 is editing service metadata across every provider
  file, including this one, and the two must not collide.
