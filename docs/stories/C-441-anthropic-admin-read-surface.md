---
id: C-441
title: "Widen the Anthropic Admin service to the members, workspace and invite reads it does not have"
pillar: Spec
status: done
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

- [x] The `admin` service gains the Admin API's remaining **read** operations: organization members
      (list, and get by id), workspace members (list for a workspace, and get by id), one workspace
      by id, and outstanding invites (list). Name them in the shipped style —
      `anthropic-<thing>-<verb>`, unique across the service's one member namespace.
      → `anthropic-organization-members-list`, `anthropic-organization-member-get`,
      `anthropic-workspace-get`, `anthropic-workspace-members-list`,
      `anthropic-workspace-member-get`, `anthropic-invites-list`. The list/get pairs mirror the
      shipped `anthropic-models-list`/`anthropic-model-get` plural-vs-singular convention.
- [x] Every added operation is a `GET`, declares `auth = [{ credentials = ["anthropic.admin_key"] }]`,
      `risk = "low"`, `idempotency = "idempotent"`, and an `[operations.quirks.error_envelope]` with
      the same two pointers the shipped three use.
      → `anthropic_admin_surface.rs::every_admin_operation_is_an_authenticated_idempotent_read`,
      which asserts all four over the whole nine-operation service rather than the six new ones.
- [x] Every list declares `first_id`, `last_id` and `has_more` in its response schema, each
      documented as unusable for paging here, and says in its `description` that it is unpaginated.
      → `anthropic_admin_surface.rs::every_admin_list_declares_its_unusable_cursor_fields`. Writing
      it surfaced that the **shipped** `anthropic-workspaces-list` and `anthropic-api-keys-list` had
      bare `first_id`/`last_id` with no description at all; both are now brought up to the
      convention, since the item says *every* list.
- [x] **Every field naming or contacting a person carries the personal-data sentence** the three
      cited providers use. A test or a grep in the story's Progress shows it holds for each.
      → both: `anthropic_admin_surface.rs::the_fields_that_name_or_contact_a_person_say_so` pins all
      nine locations by JSON Pointer, and the grep is in Progress below.
- [x] **Nothing is invented.** `specs/anthropic/2023-06-01-excerpt.yaml` is an excerpt and does not
      describe the Admin API, so these are hand-authored like the shipped three. Any field not known
      with confidence is left out or left untyped with a note saying so — never guessed into a
      `required` list. This is [C-126](C-126-response-schema-coverage.md)'s rule and it is the one
      most likely to be broken here.
      → every field was read off Anthropic's own Admin API reference in-session (six pages, listed
      in Progress); `allowed_inference_geos` is declared with a description and **no `type`**,
      because the vendor documents it as either an array or the string `unrestricted`.
- [x] No POST, PATCH or DELETE is added.
      → `anthropic_admin_surface.rs::the_anthropic_connector_declares_no_write`, scoped to the whole
      connector rather than the `admin` service.
- [x] `cargo run -p connector-cli -- build` succeeds and `-- diff` shows **only** the intended
      additions — no other provider's artifacts move.
      → `diff` reports `951 artifacts up to date (54 providers checked)`; the 12 files `build`
      wrote are the Anthropic per-provider artifacts plus the four whole-catalogue ones.
- [x] The workspace gate is green: `cargo test --workspace`, `cargo fmt --all --check`, and whatever
      else `AGENTS.md` names.
      → run with `--no-fail-fast`, per AGENTS.md § Validation; `cargo fmt --all --check` is silent.
- [x] **Failing-first:** a test that fails before the change and passes after. `babelforce_coverage.rs`
      is the shape to copy — assert the `admin` service's exposed operation set is exactly the
      expected list, so the count and the names are both pinned.
      → `crates/connector-spec/tests/anthropic_admin_surface.rs`, 4 of 6 tests red at the merge base
      `cdaabce` and all 6 green after. The set assertion is a two-way `BTreeSet` difference, so a
      rename that holds the count constant is caught.

## Progress

**Landed** on `impl/C-441`, branched from `cdaabce`.

Six operations added to the `admin` service, taking it from three reads to nine. Every field in
every response schema was read off Anthropic's own Admin API reference during implementation rather
than recalled — `users/list-users`, `users/get-user`, `workspaces/get-workspace`,
`workspace_members/list-workspace-members`, `workspace_members/get-workspace-member` and
`invites/list-invites` under `platform.claude.com/docs/en/api/admin-api/`. The `claude-api` skill
was loaded first and turned out **not** to document the Admin API at all (it covers the Messages API
and Managed Agents), which is why the vendor reference is the cited source.

The personal-data grep this story's Acceptance asks for, run after the change:

```
$ grep -o 'Identifies a named person — read it for what the calling flow needs and do not persist it beyond that' providers/anthropic.toml | wc -l
9
```

Nine occurrences for nine person-bearing fields: `id`/`name`/`email` on both organization-member
operations, `user_id` on both workspace-member operations, and `email` on the invites list. (Note
`grep -c` reports 5 here and is the wrong tool — the schemas are inline tables, so it counts lines
rather than occurrences.) A companion test,
`no_example_person_is_invented_anywhere_in_the_file`, asserts the file contains no `@` and no
specimen name, following `providers/docusign.toml`'s rule that a personal-data field earns no
example value; the vendor's reference prints one in every sample and none of it was copied.

**Two corrections to this story's own text**, both verified in-session:

- The personal-data sentence is carried by `providers/bitbucket.toml` (3×, plus 2 variants) and
  `providers/discord.toml` (2×, plus 1 variant) — **not** by the three providers cited above
  §"The part to get right". `providers/jira.toml` contains no personal-data language whatsoever
  (`grep -i 'personal data\|named person'` returns nothing), and `providers/docusign.toml` uses a
  *different* wording, `"Personal data — the signer's own name"`. The cited line numbers point at
  unrelated declarations: `jira.toml:362` is `issue_key`'s `schema`, `docusign.toml:299-301` is a
  `templateId` body parameter. The convention is real and the quoted sentence is verbatim correct;
  only the citations are stale.
- `providers/anthropic.toml:99-104` (the read-only rationale) is quoted accurately, but its line
  numbers moved with this change.

**One shipped test required a deliberate edit**:
`crates/connector-flux/tests/anthropic_connector.rs::the_curated_operation_set_is_the_one_the_story_selected`
pinned the curated set as a five-element literal. That is the test working as designed — its whole
purpose is to make a change to the operation set a conscious edit — so the literal now lists all
eleven and its doc comment records that the charter claim it protects is about *inference*, not
about size.

**Not done, and deliberately**: the `[[services]]` block's `description` still reads "organization
info, workspaces, and API keys — read-only", which now understates the service. C-153 owns that
block in the commit this branched from and the two must not collide, so it is left for the
coordinator.

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
