---
id: C-75
title: Ship the Airtable connector
pillar: Spec
status: in-progress
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · `fields` envelope · base and table in the path
---

# Ship the Airtable connector

## Goal
Ship the spreadsheet-database everyone actually stores their operational data in: record
read, create and update, addressed by base, table and record id.

## Acceptance
- [x] `providers/airtable.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [x] `base_url = "https://api.airtable.com"`, `vendor = "Airtable"`, and a `[[auth]]` entry with `scheme = "bearer"` over `AIRTABLE_ACCESS_TOKEN`, named by `default_auth`.
- [x] A curated set of roughly four over `/v0/{baseId}/{tableIdOrName}`: record get, record
      create, record update, record delete.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [x] `cargo run -p connector-cli -- build` emits `connectors/airtable.flux` and
      `connectors/airtable.connector.toml`, both committed, and a second build is byte-identical.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [x] **The `fields` envelope is declared through `wire` paths** — Airtable wraps a record's values in
      `{{"fields": {{…}}}}`, and the flat form is rejected.
- [x] `{baseId}` and `{tableIdOrName}` are **path parameters, and the story states why that is safe**:
      path values are as unencoded as query values today, and Airtable ids are restricted to
      `[A-Za-z0-9]`. A table *name* in the path is not safe by that argument — either require the id
      form or say what happens with a name containing a space.

## Progress
- Done. `providers/airtable.toml` ships 4 operations on the single-record surface; the build emits
  `connectors/airtable.flux`, `connectors/airtable.connector.toml`, four
  `crates/catalog/ops/airtable/*.flux` renderings, `crates/catalog/src/generated/airtable.rs` and a
  regenerated `web/public/catalog.json`. A second build writes nothing (`12 providers, 107 artifacts
  up to date`) and `diff` reports `107 artifacts up to date (12 providers checked)`.
- The two hand-maintained indexes were edited by hand, as the boundary in AGENTS.md requires:
  `crates/catalog/src/generated.rs` (`mod airtable` and the `PROVIDERS` entry, in id order) and
  `crates/connector-spec/tests/shipped_providers.rs::operation_selection_stays_curated`
  (`("airtable", 4)`).
- `crates/connector-flux/tests/airtable_connector.rs` is the connector's own gate: 11 tests, of which
  the two load-bearing ones are `every_airtable_request_body_is_wrapped_in_the_fields_envelope`
  (asserted on the IR *and* on the exact emitted binding `$payload = { fields: $cell_values }`) and
  `every_airtable_path_value_is_alphanumeric_by_declaration`, which takes each declared `pattern`
  apart and checks the only admissible character class is `[A-Za-z0-9]`.
- `crates/connector-cli/tests/shipped_providers_build.rs::airtable_publishes_one_host_and_no_credential_in_its_module`
  is the pipeline half of the host/credential claim, following C-73's precedent for the same clause.

### The table-name question, settled
`{tableIdOrName}` is offered **in its id form only**. The parameter is named `table_id`, not
`table_id_or_name`, and its schema declares `pattern = "^tbl[A-Za-z0-9]+$"` — a pattern no table name
can satisfy. The argument for interpolating it unencoded is the charset: every Airtable id is a
three-letter kind prefix followed by alphanumerics (`app…`, `tbl…`, `rec…`), and percent-encoding is
the identity function on `[A-Za-z0-9]`.

What happens with a name containing a space, stated plainly because the `pattern` is a declaration and
not yet an enforcement: **nothing rejects it.** The emitted Flux does not validate `pattern` — no story
has built that — so a caller passing `Tasks Backlog` would have it interpolated verbatim and Airtable
would answer 404 for a request line with a raw space; a name containing `/` (`A/B tests`) would address
a different route entirely; one containing `?` would truncate the path and start a query string. All
three are failures rather than corruption of another tenant's data, which is why the id-only contract
is shippable today. Enforcement arrives with C-30's encoding work or a future `pattern` check, and the
declaration is what those will read.

### Schema gaps recorded in the provider file
- **The cell-value object cannot be typed.** The keys inside `fields` are the base's own column names,
  so the envelope is declared as the wire path of a single opaque `{"type": "object"}` (`cell_values` →
  `fields`) rather than as a `fields.` prefix on typed fields the way asana declares `data.`. Airtable
  does publish a per-base schema (`GET /v0/meta/bases/{baseId}/tables`), but a connector is compiled
  once and a base's schema changes at runtime, so there is no build-time moment at which it could be
  read.
- **The envelope must be restated per operation**, the same gap `providers/asana.toml` records: no
  connector-level `body_envelope`/`response_envelope` field exists, so a fifth operation added without
  the `fields` wire path is silently wrong and only this connector's own test catches it.
- **Two error shapes, one `ErrorEnvelope`.** The structured form
  `{"error": {"type": …, "message": …}}` is what the declared pointers name, but Airtable also answers
  some 4xx with the bare string `{"error": "NOT_FOUND"}` — which a record get for a missing id actually
  returns — and against that body both pointers resolve to nothing.
- **The rate limit's *scope* is not declarable.** Airtable publishes a hard 5 requests/second **per
  base** with a 30-second lockout, but `RateLimit`'s `bucket` is a static string while the limited
  thing is identified by `base_id`, a runtime path value. A per-operation default bucket would split
  one base's budget four ways and exceed the limit while each bucket believed it was compliant — the
  fail-open direction — so no `quirks.rate_limit` is declared. Nothing consumes the field yet anyway
  (C-12).

## Notes
- The table-name case is the first place in this repo where the unencoded-path-parameter question
  has a real answer rather than a lucky one (`openai-model-get` was safe by the vendor's id charset).
- Deliberately excluded pending C-30: `listRecords` with its `filterByFormula`, which is both a query
  value and a formula language.
- The full excluded set, pending **C-30** (every item is a query value):
  - `listRecords` (`GET /v0/{baseId}/{tableIdOrName}`) and with it every read of more than one record.
    `filterByFormula` is the worst query value in this fleet — `AND({Status}='Open', FIND('&', {Notes}))`
    is ordinary syntax made of the exact characters that corrupt an unencoded query string — and
    `view`, `sort[0][field]`, `fields[]` and the `pageSize`/`offset` pair go with it. `offset` is an
    opaque server-issued token a caller must never construct, the same shape as a Slack cursor.
  - The read options on the single-record get: `cellFormat`, `timeZone`, `userLocale` and
    `returnFieldsByFieldId`. The shipped get therefore returns Airtable's documented default — JSON
    cell values keyed by column name.
  - The metadata API (`/v0/meta/bases`, `/v0/meta/bases/{baseId}/tables`), which pages with the same
    `offset` token and is a different resource tree from the record surface.
- Left out pending **C-56** (an omitted optional body field travels as an explicit `null`):
  - `typecast` on create and update — the flag that lets Airtable coerce a string into a select option,
    a linked record or a date. `{"typecast": null}` is a type error rather than an omission, so a cell
    value must already be in the exact JSON form its column expects.
  - `returnFieldsByFieldId` on create and update, which would key the response by `fld…` id.
- Also excluded, and **not** because of C-30 or C-56: `PUT /v0/{baseId}/{tableIdOrName}/{recordId}`,
  Airtable's record *replace*. It clears every column the caller did not send, so under C-56 it would
  be a data-loss write on every call that did not restate the whole row — the same reasoning that
  excluded jira's issue update. The shipped update is the sparse `PATCH`, whose semantics make the C-56
  gap cost nothing: the columns named in `fields` are exactly the ones that change. The batch forms of
  create/update/delete are excluded too — a `records` array is the same untyped shape as `fields` with
  an extra dimension, and nothing can declare that a batch caps at 10.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
