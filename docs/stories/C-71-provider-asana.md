---
id: C-71
title: Ship the Asana connector
pillar: Spec
status: in-progress
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · every body and response wrapped in `data`
---

# Ship the Asana connector

## Goal
Ship a clean, well-documented task API — and with it the first connector whose every request
body and response is wrapped in a `data` envelope, which is a real test of C-29's wire paths.

## Acceptance
- [x] `providers/asana.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [x] `base_url = "https://app.asana.com"`, `vendor = "Asana"`, and a `[[auth]]` entry with `scheme = "bearer"` over `ASANA_ACCESS_TOKEN`, named by `default_auth`.
- [x] A curated set of roughly five, path-addressed over `/api/1.0`: task get, task create,
      task update, story (comment) add, project get.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [x] `cargo run -p connector-cli -- build` emits `connectors/asana.flux` and
      `connectors/asana.connector.toml`, both committed, and a second build is byte-identical.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [x] **The `data` envelope is declared through `wire` paths, not flattened.** Asana wraps every
      request body in `{{"data": {{…}}}}` and every response in `{{"data": …}}`. C-29 added wire paths for
      exactly this; a connector that ignores the envelope produces requests Asana rejects.
- [x] The response envelope is recorded so a consumer knows the payload is at `/data`.

## Progress
- **Shipped 2026-07-30.** `providers/asana.toml` declares 5 operations — `asana-task-get`,
  `asana-task-create`, `asana-task-update`, `asana-task-story-add`, `asana-project-get` — all under
  `/api/1.0`, all addressed by `gid` path parameters, all authenticated by one bearer over
  `ASANA_ACCESS_TOKEN`. `cargo run -p connector-cli -- build` writes 9 new artifacts and `diff`
  reports `67 artifacts up to date (7 providers checked)`; a second build writes nothing.
- **The envelope is declared, both halves.** Every body field carries a `data.`-prefixed `wire` path,
  so the emitted payload is `{ data: { … } }`; every operation records
  `response_schema = {type: object, required: [data], properties.data: {type: object}}`, which
  `web/public/catalog.json` publishes verbatim. `crates/connector-flux/tests/asana_connector.rs`
  asserts both on the IR and on the emitted text — the payload check is
  `$payload = { data: {`, because a flattened body still parses, analyzes and is canonical.
- **What C-56 cost this connector**, field by field, so the follow-up has a list to work from:
  `asana-task-create` loses `notes`, `projects`, `assignee`, `due_on`, `parent` and `followers`;
  `asana-task-update` loses `name`, `notes`, `due_on` and `assignee`, which is why it sets exactly
  one field (`completed`) and the id stays endpoint-shaped so C-56 can widen it without renaming a
  published op; `asana-task-story-add` loses `html_text` and `is_pinned`. Two declared-required
  fields are **not** vendor-required — `data.name` on create and `data.completed` on update — and
  each says so at its declaration, because Asana's body has no required member at all.
- Two schema gaps are recorded in the file's header rather than worked around: the `data` envelope is
  a property of the *connector* but can only be restated per operation, so a sixth operation added
  without it is silently wrong in both directions; and Asana returns an **array** of errors, while
  `ErrorEnvelope.message_pointer` addresses one location, so `/errors/0/message` names only the
  first. No `rate_limit` is declared because Asana's published limit is per account tier and nothing
  here knows the tier.

## Notes
- Asana is the best available proof that C-29's nested-body support works on a real vendor rather
  than a fixture — babelforce's bodies motivated it, but the envelope here is universal across every
  operation.
- Deliberately excluded pending C-30: `opt_fields` (which every Asana endpoint accepts and which is
  the documented way to widen the returned field set), `limit`/`offset` paging with its opaque
  server-issued `offset`, `GET /tasks` and its filters, and
  `GET /workspaces/{workspace_gid}/tasks/search`.
- Deliberately excluded pending C-56, because an omitted optional body field travels as an explicit
  `null` that Asana rejects: every optional member of `data` on the three writes — see the Progress
  list above for the exact fields.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
