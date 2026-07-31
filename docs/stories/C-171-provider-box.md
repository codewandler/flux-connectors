---
id: C-171
title: Ship the Box connector
pillar: Spec
status: in-progress
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "folder and file ids are opaque strings where `0` is the magic root — a sentinel value a model will guess wrong without being told"
---

# Ship the Box connector

## Goal

Add Box to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A magic root id.** Box's root folder is literally `0`. A curated connector has to say that, or every first call fails.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <access_token>`.

**Curated operation set (a starting point, not a mandate):** list a folder's items, get file info, get folder info, create a folder, copy a file

## Hazards specific to this one

Keep the download endpoint out: it answers `302` to a signed URL, and redirect-following is not a declared behaviour of this pipeline. Say so rather than shipping an operation whose success depends on a client setting nobody declared.

## Acceptance

- [x] `providers/box.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
- [x] A `verify` operation that is a read and runs unattended.
- [x] `crates/connector-flux/tests/box_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses.
- [x] **Failing-first test:** the contract test must fail before `providers/box.toml` exists.
- [x] The scoped gate is green: `build --provider box`, `diff --provider box` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding.

## Notes

- **Charter fit.** Box is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/box.rs` is **not** in that set and is yours to commit.

## Progress

- **2026-07-31 — shipped.** `providers/box.toml` (6 operations), `crates/connector-flux/tests/box_connector.rs`
  (8 tests), plus the per-provider generated artifacts (`connectors/box.flux`,
  `connectors/box.connector.toml`, `crates/catalog/ops/box/*.flux`, `crates/catalog/src/generated/box.rs`).
- **Dispatch/worktree note.** This wave was dispatched before its story files were committed, so this
  worktree branched from `939da49` and initially had no `docs/stories/C-171-provider-box.md` to read.
  Merged `main` (now at `95d6674`, containing this file and 23 sibling provider stories) with
  `git merge --no-ff main` before doing anything else, per the coordinator's correction. `BASE_PROOF`
  below is taken at that merge base.
- **The root-folder sentinel is declared twice per parameter, not once.** Every `folder_id` /
  `parent_id` parameter (`box-folder-get`, `box-folder-items-list`, `box-folder-create`,
  `box-file-copy`) carries `default = "0"` on its JSON Schema *and* names "0" and "root" in its
  `description`. Both halves are asserted by
  `the_root_folder_sentinel_is_declared_on_every_folder_id_parameter`, mirroring the two-halves shape
  of C-107's `Notion-Version` test: a model that only reads the composed `input_schema` sees the
  default; a model that only reads prose sees the sentence.
- **The download endpoint (`GET /2.0/files/{file_id}/content`) is deliberately not shipped**, per the
  hazard: Box answers `302` to a signed URL on a separate host, and nothing in this pipeline declares
  or performs redirect-following. `the_download_endpoint_is_not_shipped` pins that no emitted path ends
  `/content`. `box-file-get` and `box-folder-get` return metadata only, and their `description` says so
  in the same words a model would need to not assume otherwise.
- **Rust-keyword finding, not a blocker.** `box` is a reserved Rust keyword, and the coordinator asked
  this be checked explicitly. Traced at `crates/connector-cli/src/catalog.rs`: `module_ident` already
  has a `RUST_KEYWORDS` table (includes `box`) and escapes any match to `r#box` when rendering
  `pub(crate) mod r#box;` / `&r#box::PROVIDER` into `crates/catalog/src/generated.rs` — asserted by
  that file's own `a_keyword_provider_name_is_escaped` test. `crates/catalog/src/generated/box.rs`
  itself is unaffected: its internal identifiers (`PROVIDER`, `AUTH`, `OPERATIONS`) are fixed names,
  not derived from the provider id, and the Flux side never turns the provider id into an identifier
  at all — `connectors/box.flux` names it only in a `# Provider: box` comment. So nothing here needed
  escaping by hand; the coordinator's full build will emit the escaped `mod r#box;` line into the
  fenced index automatically. Confirmed by reading the code, not run, since a full build is out of
  scope for this gate.
- **Unverified / not shipped**, named per the story's instruction to leave out rather than guess:
  Box's OAuth2 token exchange (`POST /oauth2/token`), search (`GET /2.0/search` — its `query` parameter
  is the same injectable free-text shape C-30 excludes elsewhere), collaborations, comments, metadata
  templates, retention policies, legal holds, shared links, uploads, file version history, trash, and
  users/groups administration. All are well-documented, stable Box endpoints; they are left out for
  scope, not uncertainty about their shape — the only thing genuinely excluded for a hazard rather than
  scope is the download endpoint, above.
- **Query parameters excluded entirely** (`offset`, `limit`, `fields` on `box-folder-items-list`),
  the same choice `providers/notion.toml` made for the same reason (C-30, no percent-encoding).
  `box-folder-items-list` therefore returns Box's first page only.
- **Gate:** `build --provider box` wrote 9 artifacts; `diff --provider box` reports
  `9 artifacts up to date (1 provider checked)`. `cargo build --workspace`, `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo fmt --all --check` are all clean. `cargo test --workspace
  --no-fail-fast` leaves **exactly the eight whole-catalogue tests red** that `AGENTS.md` tabulates,
  across the same five binaries (`connector-catalog::embedded_operations` ×2,
  `connector-cli::catalog_artifacts` ×1, `connector-cli::readme_snippet` ×1,
  `connector-cli::service_units` ×2, `connector-cli::site_catalog` ×2), and nothing else — grepped
  for `FAILED$` across the full run: exactly 8 matches, all named. Not silenced; the coordinator's
  full build resolves them.
- **Board not regenerated** and no whole-catalogue artifact touched — both coordinator-owned. `status`
  is `in-progress` here and needs `/track:done` plus `/track:board` at integration.
- Branch is `impl/C-171`, created from `939da49` and carrying one merge commit (`21a80f3`, `--no-ff
  main`) that brought in the 25 committed story files, followed by the implementation commit(s).
