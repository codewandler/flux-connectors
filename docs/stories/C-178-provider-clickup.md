---
id: C-178
title: Ship the ClickUp connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "`Authorization: <pk_...>` raw, and the resource tree is four levels deep (team → space → folder → list) before a task"
---

# Ship the ClickUp connector

## Goal

Add ClickUp to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A deep containment hierarchy.** A task lives under a list under a folder under a space under a team. A curated connector has to decide how much of that ladder the caller must know.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: <token>`, raw — no scheme word.

**Curated operation set (a starting point, not a mandate):** list tasks in a list, get a task, create a task, update a task, list a space's folders

## Hazards specific to this one

Same raw-Authorization question as [C-175](C-175-provider-launchdarkly.md); read whichever landed first. Do not ship every rung of the hierarchy as its own operation just because it exists — the epic's rule is that a connector earns its place, and five navigation endpoints teach nothing.

## Acceptance

- [x] `providers/clickup.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/clickup.toml`, six
      operations: `clickup-team-list` (verify), `clickup-space-folder-list`, `clickup-list-task-list`,
      `clickup-task-get`, `clickup-task-create`, `clickup-task-update`.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every `[[operations]]` block in `providers/clickup.toml`
      declares `risk`/`idempotency`; the two writes are `medium`/`non_idempotent` with the reasoning in
      the comment above them.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → `providers/clickup.toml`'s `[[config]]` block (`token`), `secret = true` against
      `binds = "credential.clickup.token"`.
- [x] A `verify` operation that is a read and runs unattended. → `verify = "clickup-team-list"`,
      `GET /team`, no parameters.
- [x] `crates/connector-flux/tests/clickup_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. → asserts the
      bare-`Authorization`-header shape and that the curated set stops short of two specific navigation
      rungs (team's spaces, folder's lists).
- [x] **Failing-first test:** the contract test must fail before `providers/clickup.toml` exists. → see
      `BASE_PROOF` below.
- [x] The scoped gate is green: `build --provider clickup`, `diff --provider clickup` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly eight, plus the
      documented ninth (`the_recorded_floor_is_the_measured_figure`); see the implementor's report.

## Progress

- Confirmed via live fetches against `developer.clickup.com/reference/*` (not guessed): `GET /team`,
  `GET /space/{space_id}/folder`, `GET /list/{list_id}/task`, `GET /task/{task_id}`,
  `POST /list/{list_id}/task`, `PUT /task/{task_id}` — paths, methods, and the response/body field
  names used in `providers/clickup.toml`'s schemas.
- **Curated to six operations, not the five the story starts from**, because `verify` must name a
  declared operation and the only parameterless read ClickUp's API offers is `GET /team`
  (`clickup-team-list`). Deliberately did **not** add a "list a team's spaces" operation (a space id is
  read off ClickUp's own UI URL, the same argument `providers/gitlab.toml` makes for a numeric
  `project_id`) or a "list a folder's lists" operation (`GET /space/{space_id}/folder` already nests
  each folder's lists inline, so a separate rung would refetch data the first call already returned).
  See the header comment in `providers/clickup.toml` for the full argument.
- **Unverified / left out, named rather than guessed:** the array-valued query filters on
  `GET /list/{list_id}/task` (`statuses[]`, `assignees[]`, `watchers[]`, `tags[]`,
  `custom_fields[]`) — this pipeline's query-parameter model is one name to one scalar value, and
  there is no way to express a repeated `key[]=a&key[]=b` query key without a connector-specific
  encoding rule. `custom_task_ids`/`team_id` alternate addressing and the opaque `custom_fields` JSON
  filter on `clickup-task-get` are excluded for the same reason. `assignees`/`watchers` on
  `clickup-task-update` are excluded because ClickUp takes them as `{add, rem}` deltas naming specific
  people, not a plain value.
- The exact shape of `folders[].lists[]` (bare ids vs. nested list objects) was not pinned to a single
  schema — described loosely as "the lists inside this folder, each with at least `{id, name}`" — one
  fetched summary described it as bare ids while ClickUp's public docs elsewhere show nested objects;
  rather than guess, the response schema documents intent without asserting a strict `items` shape a
  future drift check could fail on.

## Notes

- **Charter fit.** ClickUp is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
  non-goals exclude technology adapters and the inference path. If authoring reveals it is really a
  technology rather than a service, stop and say so rather than shipping it.
- **Provenance is hand-authored and drift is undetectable by machine**, the caveat zendesk, freshdesk,
  github and notion already carry. Record it in the TOML's header comment.
- **Do not invent an endpoint.** A confident set of four operations beats a guessed set of ten — the
  repository's rule is that a loud refusal beats plausible-but-incorrect output, and a hallucinated
  path ships as a `404` a contract test cannot catch. Where you are unsure of a path, shape or required
  field, leave the operation out and name it in `## Progress` as unverified.
- **No credential value, and no realistic-looking `example` on a secret field.** A placeholder shaped
  like a real token has already tripped GitHub's push protection and blocked a release.
- Whole-catalogue artifacts are coordinator-owned: `crates/catalog/src/generated.rs`,
  `web/public/catalog.json`, `web/public/v1/**`, `assets/readme-snippet-*.svg`. The per-provider
  `crates/catalog/src/generated/clickup.rs` is **not** in that set and is yours to commit.
