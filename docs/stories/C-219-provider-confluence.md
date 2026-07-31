---
id: C-219
title: Ship the Confluence connector
pillar: Spec
status: in-progress
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: same host and same credential as the shipped `jira` connector (`{site}.atlassian.net`, Basic email+token). Two connectors, one authority — the credential-addressing model's first real collision"
---

# Confluence — a second Atlassian product sharing an authority with Jira

## Goal

Ship a curated `confluence` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

`jira` already ships with authority `com.atlassian.<site>` and a Basic email+token credential.
Confluence uses **the same host, the same account and the same token**, at a different path
(`/wiki/api/v2`).

This is the first time two connectors would legitimately share one authority, and it is the probe
the credential-addressing model has never had: does an operator who has connected Jira have to paste
the same token again for Confluence, or does the address resolve to the value they already supplied?
[C-90](C-90-credential-addressing-epic.md)'s whole point is that an address is a *place*, not a
per-connector copy — this is where that either pays off or is revealed as untested.

## Acceptance

- [x] `providers/confluence.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [x] A `verify` operation that is an argument-free read and runs unattended.
- [x] `crates/connector-flux/tests/confluence_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [x] **Failing-first test:** the contract test must fail before `providers/confluence.toml` exists.
- [x] The scoped gate is green: `build --provider confluence`, `diff --provider confluence` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- **The answer to "does it reuse Jira's credential" is the story's real deliverable**, more than the
  operations are. Whatever it turns out to be, assert it in the contract test rather than describing
  it.
- Decide deliberately between a separate `confluence` connector and a second **service** on an
  `atlassian` connector. C-49 established services for exactly this shape (`google` has gmail,
  calendar, drive on one authority) — but `jira` already ships standalone, so a service split would
  mean moving it. Record the reason either way; do not let the existing layout decide by default.
- Confluence's v2 API paginates with an opaque cursor in a `Link` header. If that cannot be
  expressed, say so rather than shipping an operation that silently returns one page.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.

## Progress

**Complete on `impl/C-219`.** Six operations, four reads and two writes, against Confluence Cloud
REST v2 (`/wiki/api/v2` on `{site}.atlassian.net`), read from Atlassian's OpenAPI document
`developer.atlassian.com/cloud/confluence/openapi-v2.v3.json` on 2026-07-31 and cited in the
provider header.

### The probe's answer: **yes, the operator pastes the token again**

Asserted, not described, in
`confluence_connector.rs::the_two_atlassian_connectors_do_not_share_a_credential_address`, which
loads *both* shipped provider files and renders both addresses:

```
tenants/<tenant>/com.atlassian.jira/api_token
tenants/<tenant>/com.atlassian.confluence/api_token
```

Same tenant, same leaf (`api_token`), and the **authority segment is the entire difference**. The
addressing model is not broken — `Connector::credential_ref_for` (`ir.rs:1166-1178`) keys on
`authority` + leaf, and C-92 derives an `authority` per *product*, so two products can never collide
however identical the token physically is. The finding is that C-90's "an address is a place, not a
per-connector copy" is true **within** a connector and was never wired to reach **across** two.
Nothing before this story had two connectors close enough together to expose that.

### The layout decision: standalone connector, not a service on `atlassian`

Recorded with its cost in the provider header, and the rejected branch is *measured* rather than
merely asserted —
`confluence_connector.rs::a_service_split_would_have_shared_the_credential_and_is_still_refused`
constructs `tenants/<tenant>/com.atlassian/api_token` and shows one `atlassian` connector with
`jira` + `confluence` services **would** have delivered single-paste sharing.

It is refused because `com.atlassian.jira` is a *published* authority and AGENTS.md is flat that a
published address is never repointed: folding Jira in would move every Jira `gid`, every rendered
address, and every already-provisioned tenant credential path. That is a migration story with a
deprecation window, and this is a provider story that must leave `providers/jira.toml` untouched.

### The curation constraint, and what it cost

`body-format` is a **query** parameter, and this connector declares none (C-30). A v2 read that
omits it returns `"body": {}`, so **this connector cannot read any content body — not a page's, not
a comment's.** It is therefore curated as a connector that *navigates and writes* a site rather than
one that reads it back, and every read says so in the `description` a model receives
(`no_confluence_read_promises_page_content_it_cannot_fetch` asserts that).

Two exclusions follow directly and are named in the header:

- **`PUT /pages/{id}`** — the vendor requires `version.number` = current + 1 plus a full `title` and
  `body` on every update, so an update is a read-modify-write. The "read" half is unavailable, so
  every update would blindly replace the page's content. Same data-loss reasoning that excludes
  Jira's issue update.
- **`GET /pages/{id}/footer-comments`** — a comment is *only* its body, so with `body-format`
  unavailable the operation cannot do the one thing it is for. (A page read still yields a title, a
  version and a `webui` link, which is why it survives and the comment read does not.)

Pagination is declared nowhere: Confluence's cursor is doubly inexpressible (`cursor_param` is a
query parameter, and `_links.next` is a *relative URL* rather than the bare token
`next_cursor_pointer` is defined to locate). Each list read states the first-page-of-25 truncation
in its own description.

### Two vendor-fixed values are pinned rather than asked for

`status = "current"` and `body.representation = "storage"` are pinned with a JSON Schema `const`,
which `op.rs:486-494` sends as a literal and never declares as a caller argument. Verified on the
emitted Flux: `op confluence-comment-add(page_id: String, body: String)` — two arguments, with
`body_representation = "storage"` bound as a literal. This matters beyond tidiness: Confluence
stores a mislabelled representation verbatim, so a model sending Markdown labelled `storage` would
get a `200` and a corrupted page, not an error.

### Follow-up worth a story: the missing third case of C-55

C-55 gave the pipeline vendor-constant **headers** (`const_headers`) and vendor-constant **body**
fields (the `const` pin above) and stopped there. There is no way to declare a vendor-constant
**query** parameter, and `body-format=storage` is exactly one — a value this connector would always
send and a caller would never choose. That mechanism, *not* C-30's percent-encoder, is the smallest
change that would give this connector its content reads back and unblock `PUT /pages/{id}`: a
constant query parameter needs no encoding machinery, because no caller value passes through it.
Baking `?body-format=storage` into `path` was considered and refused — it would break the day a real
query parameter was added beside it, and `no_confluence_module_assembles_a_query_string` now refuses
a `?` in any path.

### Gate

Scoped gate green: `build --provider confluence` (9 artifacts), `diff --provider confluence` →
`9 artifacts up to date (1 provider checked)`, `cargo build --workspace`, clippy with `-D warnings`,
`cargo fmt --all --check`. All 16 contract tests pass.

**Exactly eight whole-catalogue staleness tests are red across five binaries**, matching AGENTS.md's
table name-for-name; the ninth (`the_recorded_floor_is_the_measured_figure`) stayed **green**, so
this story fits inside the coverage slack alone. No whole-catalogue artifact was regenerated — the
coordinator resolves all eight at integration.
