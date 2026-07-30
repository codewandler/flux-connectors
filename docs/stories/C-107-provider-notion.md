---
id: C-107
title: Ship the Notion connector
pillar: Spec
status: ready
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "blocked on C-55, now measured rather than predicted — a const-pinned Notion-Version emits as a caller-overridable parameter with the const dropped, and Notion REJECTS every request without the header"
---

# Ship the Notion connector

## Goal
Pages, databases and search — and the connector that makes a `ready` story unavoidable.

## Acceptance
- [ ] A curated operation set: retrieve a page, query a database, create a page, search.
- [ ] **`Notion-Version` is sent on every request.** Notion rejects a request without it, so this is
      not a nicety — it is the connector working at all.
- [ ] Auth: a bearer integration token.
- [ ] A `[[config]]` surface and a `verify` operation.
- [ ] A per-provider contract test asserting the version header reaches every emitted operation.

## Progress
- **2026-07-30 — attempted, blocked on C-55 at the emitter. Nothing shipped, deliberately.** The
  dependency this story's Notes predicted is real, and it was measured rather than read: a probe
  `providers/notion.toml` declaring the version header the only way the schema allows —

  ```toml
  [[operations.params.header]]
  name = "Notion-Version"
  required = true
  schema = { type = "string", const = "2022-06-28" }
  ```

  — built cleanly under `build --provider notion` and emitted

  ```
  op notion-page-get(page_id: String, Notion_Version: String) -> Any
    …
    response = http.request(headers: { "Notion-Version": Notion_Version }, method: "GET", url)
  ```

  The `const` is dropped entirely. What ships is a required `String` argument on the tool contract
  that **a model must guess on every call and any caller may set to anything** — precisely the
  outcome this story's Notes say to refuse. Cause, confirmed at `path:line` rather than inferred:
  `crates/connector-flux/src/op.rs:272` chains `&header` into the declared parameter list
  *unfiltered*, while only the `body` chain gets `.filter(|b| constant(b.param).is_none())`;
  `:501-506` then emits each header param as a symbol reference. `constant()` (`:310`) reads
  `schema.const` and is never consulted for a header.
- The whole schema was re-enumerated to be sure there is no other route, matching the audit
  `providers/github.toml` already records: no `headers` table at provider, service or operation
  level; no `value`/`const` field on `param`; `api_version` is address metadata and reaches no
  header (it appears in `connector-flux` only in test fixtures); `content-type` is hard-coded in the
  emitter and is not declarable. Declaring the version through `[[auth]]` was rejected on sight — it
  is not a credential, and generated Flux carries a credential *reference*, never a value, so it
  could not carry `2022-06-28` even if the model allowed it.
- **The per-provider compile gate does not catch this, which is the point.** With the probe in place
  `every_shipped_provider_compiles` and all 12 of `shipped_providers_build.rs` **passed**. A Notion
  connector missing its version header compiles, formats, round-trips and ships — and then answers
  `400 validation_error` on every call. Only this story's own acceptance test (*"the version header
  reaches every emitted operation"*) would have caught it, and that test cannot be made to pass
  without C-55.
- The eight expected whole-catalogue red tests were confirmed with the probe in the tree
  (`cargo test --workspace --no-fail-fast`), exactly as AGENTS.md tabulates them: 2 in
  `catalog::embedded_operations`, 1 in `catalog_artifacts`, 1 in `readme_snippet`, 2 in
  `service_units`, 2 in `site_catalog`. The probe and its four generated artifacts were then removed;
  this branch changes **only this file**.
- **Board not regenerated** — `docs/stories/README.md` is coordinator-owned. `status` moved
  `ready` -> `blocked` here, so the board needs a `/track:board` run at integration.

## Curation, decided and ready to apply

Recorded so the C-55 follow-up is a transcription rather than a re-derivation. Every decision below
was checked against the loader and emitter constraints that actually exist today.

**Selected — 4 operations plus a `verify`.** Notion's API is ~30 endpoints; this is the subset that
is both wanted and honestly expressible.

| id | route | risk | idempotency | why |
|---|---|---|---|---|
| `notion-page-get` | `GET /v1/pages/{page_id}` | `low` | `idempotent` | Page **properties**, not content — see the block model below |
| `notion-database-query` | `POST /v1/databases/{database_id}/query` | `medium` | `none` | The workhorse read |
| `notion-search` | `POST /v1/search` | `medium` | `none` | Title search across shared content |
| `notion-page-create` | `POST /v1/pages` | `high` | `none` | Writes content a workspace can see |
| `notion-user-me` | `GET /v1/users/me` | `low` | `idempotent` | `verify` — a genuine GET, so it satisfies "a verify operation is a read" |

**The two POST reads are declared `medium`/`none` and that overstates them.** `check_write_metadata`
(`crates/connector-flux/src/op.rs:333`) refuses `risk = "low"` on any POST and refuses
`idempotency = "idempotent"` on POST/PATCH — correctly, since it cannot see that Notion uses POST for
a read. So `notion-database-query` and `notion-search` are pure reads that must present to flux's
approval gate as writes. This is conservative-safe rather than dangerous (it over-prompts, never
under-prompts), but it is a real loss of fidelity and it belongs in the file's header comment, not in
a commit message. Do **not** "fix" it by declaring `low` — that is a loud, correct refusal.

**Left out, each for a stated reason:**

- **`GET /v1/blocks/{block_id}/children` and `PATCH …/children` (append) — the block model.** This is
  the shape the story asks to be honest about. A block is a discriminated union of ~30 type-specific
  bodies (`paragraph`, `heading_1..3`, `to_do`, `table`, `column_list`, …) whose payloads are
  *recursive* — a block's children are blocks. `JsonSchema` here has no `$ref` and no recursion, so
  the union is not expressible; `body_schema` could take it as a free-form object, but that ships an
  untyped blob and hands the model a guess. **Reading or writing page content is therefore out of
  scope for this connector**, and the file must say so plainly rather than implying `notion-page-get`
  returns a page's text. It returns its properties.
- **`PATCH /v1/pages/{page_id}` (update properties).** The `properties` object is keyed by
  *user-defined* property names whose value shape depends on each property's type in that specific
  database. Nothing static can describe it; a `wire` path cannot address a key that does not exist
  until a tenant creates it.
- **`filter` / `sorts` on `notion-database-query`.** A recursive boolean DSL, same unexpressible
  shape as the block union. The operation ships as an unfiltered page-1 read, which is useful and
  honest; C-56's optional-body-field gap also applies, since sending `{"filter": null}` is a 400.
- **All pagination (`start_cursor`, `page_size`).** Notion's cursor is an opaque server-issued token
  and the query surface is unencoded (C-30) — the standing `zendesk-ticket-search` gap. Excluding it
  is what keeps this connector off AGENTS.md's *Intentional gaps* list.
- Comments, file uploads, database create/update, `GET /v1/users`, and the OAuth token exchange —
  out of scope for a first curated set, not blocked by anything.

**Auth and config.** One `[[auth]]` entry, `notion.token`, `scheme = "bearer"`, `env =
["NOTION_TOKEN"]`. Notion is single-host (`api.notion.com`) with no tenant subdomain, so the only
connection-level `[[config]]` field is the token itself. **No `example` on it** — a Notion token is
`ntn_` + 46 chars, and a placeholder of that shape is exactly what tripped push protection in this
repo before. Use a self-evidently fake `help` string and no `example`.

## Notes
- **Ordering edge: this depends on [C-55](C-55-constant-request-headers.md)** (*"Let a provider
  declare a constant request header"*, `status: ready`, unimplemented). There is no way to send a
  constant header today. Either C-55 lands first, or this story absorbs it — and if it absorbs it,
  it is no longer disjoint from anything else touching the emitter and must run solo.
- The alternative — declaring the version as a required parameter every caller passes — is wrong and
  should be refused: it is a constant of the connector, not an input, and a model would have to guess
  it on every call.
- **Both notes above are now confirmed, not predicted** (2026-07-30, see Progress). The "absorbs it"
  branch is not available to a fanned-out implementor: C-55 lands in `connector-spec` and
  `connector-flux`, which provider-story dispatch fences off precisely so provider stories stay
  disjoint. **Sequence it: C-55, then re-dispatch this story**, at which point the curation table
  above applies unchanged and the only open question is the spelling C-55 chooses for the header
  declaration.
- C-55's acceptance should gain Notion as a second witness alongside GitHub. GitHub is a *weak* one —
  its own file records that GitHub defaults `Accept` when absent, so nothing there is broken today
  and the test can only assert a header's presence. Notion is the case where the header's absence is
  total failure, which is the difference between version *pinning* and version *working*.

- **Unblocked.** C-55 landed `const_headers`, so `Notion-Version` is declarable as a literal.
  GitHub is the worked example in `providers/github.toml`. The curation recorded above is unchanged,
  so the re-dispatch is transcription rather than re-derivation.
