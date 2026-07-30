---
id: C-52
title: Ship the GitHub connector
pillar: Spec
status: in-progress
priority: 3
design: docs/designs/provider-operation-inventory.md
epic: connectors-v1
areas: [providers, connector-spec]
note: bearer · path-and-body surface only · listing ops wait on C-30
---

# Ship the GitHub connector

## Goal
Add `providers/github.toml` and its generated artifacts, curated to the part of the GitHub REST API
that this pipeline can express honestly today: path parameters and JSON bodies, no query strings.

## Acceptance
- [x] `providers/github.toml` is hand-authored, following the zendesk precedent. GitHub publishes
      `github/rest-api-description`, which is the `[spec]` pointer this file becomes once C-4 lands;
      record that in the header comment along with the operation set as the selection to reproduce.
      → `providers/github.toml:1-15`, which names the upstream document, says why it is not a
      `[spec]` pointer yet, and lists the five `operationId`s as the selection to reproduce.
- [x] `base_url = "https://api.github.com"`, `vendor = "GitHub"`, `[[auth]]` with
      `scheme = "bearer"` over `GITHUB_TOKEN`, named by `default_auth`.
      → `providers/github.toml:62` (`id`), `:63` (`vendor`), `:70` (`base_url`), `:78`
      (`default_auth`), `:85-89` (the bearer). Asserted by
      `crates/connector-flux/tests/github_connector.rs::the_github_connector_loads`.
- [x] A curated operation set of roughly five, each with `risk` and `idempotency`. Confirm against
      current vendor docs; the intended set is `github-repo-get` ·
      `github-issue-get` · `github-issue-create` · `github-issue-comment-add` · `github-pull-get`,
      all addressed by path parameters (`{owner}`, `{repo}`, `{issue_number}`, `{pull_number}`).
      → exactly those five, path-addressed only; count pinned by
      `crates/connector-spec/tests/shipped_providers.rs::operation_selection_stays_curated`.
      See Progress for what "confirm against current vendor docs" could and could not mean offline.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested. C-30 is not
      implemented and the emitter would emit such a value unencoded — `state`, `labels` and `q` are
      exactly the injectable shapes `zendesk-ticket-search` already demonstrates. Listing and search
      operations are therefore out of scope here and named in Notes.
      → tested in the **strong** form: zero query parameters of any type, asserted both on the IR
      (`github_connector.rs::no_github_operation_declares_a_query_parameter`) and on the emitted URL
      (`::no_github_operation_emits_a_query_string`).
- [x] **GitHub's required `Accept: application/vnd.github+json` header is either declared or reported
      as a schema gap.** If no field can express a constant, non-credential header, say so in the
      header comment and in this story's Progress, following the `SCHEMA GAP:` precedent in
      `providers/zendesk.toml` — do not smuggle it in as a parameter with a default the caller can
      overwrite unless that is genuinely what the schema means.
      → **reported as a schema gap**; it cannot be declared. `providers/github.toml:17-60` and
      Progress below. Nothing is smuggled in: the connector declares no header parameter at all.
- [x] `cargo run -p connector-cli -- build` emits `connectors/github.flux` and
      `connectors/github.connector.toml`, committed, and a second build is byte-identical.
      → both committed; the second build reported `45 artifacts up to date; nothing written` and
      `diff` reported `45 artifacts up to date (4 providers checked)`.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point tests every
      existing provider passes.
      → `github` added to the shared `SHIPPED` lists in `connector-flux/tests/shipped_modules.rs`,
      `connector-spec/tests/shipped_providers.rs`, `connector-cli/tests/{catalog_artifacts,
      shipped_providers_build,site_catalog}.rs`, so the existing gates cover it rather than a
      github-only copy of them.
- [x] `crates/catalog/src/generated.rs` gains its `pub(crate) mod github;` line and its `PROVIDERS`
      entry in id order — `crates/catalog/tests/embedded_operations.rs` fails until it does.
      → `crates/catalog/src/generated.rs:15` and `:25`, both between `freshdesk` and `zendesk`. The
      counts in `embedded_operations.rs::the_catalog_is_not_empty` moved 3→4 providers and 25→30
      operations.
- [x] `http_hosts` is `api.github.com`, never widened; no credential value in any generated artifact.
      → `base_url` is a single tenant-independent host with no templating, so the derived host is
      exactly `api.github.com` in `web/public/catalog.json` and `crates/catalog/src/generated/
      github.rs`. `GITHUB_TOKEN` appears only as a variable *name* (provider TOML and the
      catalogue's credential reference); the generated Flux carries not even the name, since auth
      injection is C-10.
- [x] `github-issue-create` and `github-issue-comment-add` write to a public surface; their `risk`
      says so.
      → both `risk = "high"`, one step above the zendesk writes, with the reasoning at
      `providers/github.toml:198-213` and the public-visibility consequence stated in each
      operation's `description` so it reaches the model too.

## Progress
- Filed 2026-07-30 under "ship up to 3 connectors, popular and useful".
- **Complete.** Five operations ship; all ten Acceptance items are ticked with evidence above. The
  full five-command gate in AGENTS.md §Validation is green, including the
  `grep -E "FAILED|error: test failed|panicked at"` diagnostic, which prints nothing.

### SCHEMA GAP: a constant, non-credential request header cannot be declared

**The `Accept: application/vnd.github+json` header GitHub asks every client to send is not declared,
because no field in the provider schema can express it.** This was checked against the loader and the
emitter rather than assumed, and the finding is recorded at `providers/github.toml:17-57`.

The whole accepted-key surface was enumerated via `connector_spec::provider::accepted_keys()`:

| object | keys |
|---|---|
| `provider` | `id`, `vendor`, `base_url`, `description`, `auth`, `default_auth`, `operations`, `spec`, `patch` |
| `operation` | `id`, `method`, `path`, `description`, `risk`, `idempotency`, `auth`, `params`, `response_schema`, `quirks` |
| `paramSet` | `path`, `query`, `header`, `body`, `body_schema` |
| `param` | `name`, `wire`, `description`, `required`, `schema` |
| `quirks` | `pagination`, `rate_limit`, `error_envelope` |

There is no `headers` table at provider or operation level, and no constant-value field on `param`.
`params.header` is the only header mechanism and the IR documents it as *"Request headers the caller
supplies"* — which is exactly what it emits.

**The `const` trick does not transfer from bodies to headers.** `providers/zendesk.toml` pins the
constant `ticket.safe_update = true` with a JSON Schema `const`, and
`crates/connector-flux/src/op.rs` filters `constant(...)` out of the declared parameter list — but
*only for `body` params* (`op.rs:273`). A `const`-pinned **header** was emitted as a probe and
produced:

```flux
op probe-get(accept: String) -> Any
  …
  $response = http.request({ headers: { accept: $accept }, method: "GET", url: $url })
```

That is a required argument every caller must pass and any caller may set to anything, with the
`const` dropped entirely (`flux_type` discards constraint keywords). It is a caller-overridable
parameter wearing a constant's clothes, which is the disguise this story's Acceptance forbids — so
C-52 declares no header at all and reports the gap loudly instead. The one constant header the
pipeline does send, `content-type: application/json`, is hard-coded in the emitter as
`JSON_MEDIA_TYPE` and is not declarable either.

**Impact, stated plainly: this is not what makes the connector non-functional.** GitHub serves
requests that omit `Accept` by defaulting to `application/vnd.github+json`, so all five operations
are well-formed as emitted. What is lost is *version pinning* — the explicit header is precisely the
protection against a future change to that default — plus any other vendor-constant header a
connector might need (an API version, a `User-Agent`, `X-GitHub-Api-Version`). The connector is
non-functional for the *same* reason every other one is: the `$auth` whole-value-replacement gap in
`docs/designs/auth-seam.md`, which `catalog.json` reports per operation as
`credential-not-injected`.

**Closing it** needs either a `headers` table on the connector and the operation, or a constant-value
field on `param` that the emitter binds as a literal the way it already binds a constant body field.
Either is a small, additive change to `connector-spec` plus a few lines in `op.rs`; neither is in
C-52's scope. This is the same class of gap as the `base_url`-to-env binding that zendesk, freshdesk
and babelforce each record — worth one story covering all of it.

### Two smaller notes for the next reader

- **Why the query test asserts zero rather than "no string-ish type".** The Acceptance says no
  string-ish or `Any`-typed query parameter. The test asserts the strictly stronger property — no
  query parameter of *any* type — because the weaker one is satisfiable by picking a narrower type
  for a value that is still injectable, and because zero is a property a reviewer can check at a
  glance. It is asserted twice, once on the IR and once on the emitted URL, so an emitter that
  synthesised a query parameter from somewhere other than `params.query` could not slip past.
- **The emitted-text assertion checks every `$url = ` line, and it has to.** Review caught the first
  version selecting only the *first* one, which made the "could not slip past" claim above false: the
  emitter binds `$url` once for the path and required query parameters and then **re-binds it once per
  optional query parameter** inside a `when` guard, with the `?` carried on a separate `$sep`
  binding (`crates/connector-flux/src/op.rs`, the `optional` loop; the shape is visible in
  `connectors/zendesk.flux`). Injecting one optional query parameter into a github operation
  therefore failed the IR assertion while *passing* the text assertion. The test now requires exactly
  one `$url` binding, no `?` on any of them, and no `$sep` binding at all — verified by re-running
  that same probe and watching both assertions fail, then reverting it.
- **The path surface is safe rather than merely untested.** `{owner}` and `{repo}` are GitHub names
  restricted to `[A-Za-z0-9._-]` and the two `*_number` parameters are integers, so no path value can
  carry a character that changes the shape of the request. That asymmetry against the query string is
  the entire selection principle for this connector, and it is why the path-and-body surface can ship
  honestly while `github-issue-list` cannot.

## Notes
- **Deliberately excluded pending C-30**: `github-issue-list` (`state`, `labels`, `assignee`),
  `github-pull-list` and every search endpoint. They are the most-wanted GitHub operations and they
  are precisely the ones the query-encoding gap makes unsafe — which makes this connector a strong
  second argument for C-30 and for flux's structured `query` map.
- GitHub Apps and fine-grained tokens both present as `Authorization: Bearer <token>`, so one auth
  method covers both; the token *type* is operator config, not connector shape.
- **Still cannot make a live call** — the `$auth` whole-value-replacement gap
  (`docs/designs/auth-seam.md`) applies here as to every connector.
