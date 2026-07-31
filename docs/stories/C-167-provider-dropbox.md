---
id: C-167
title: Ship the Dropbox connector
pillar: Spec
status: in-progress
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "every read is a POST with a JSON body — an RPC surface wearing HTTP. No shipped provider reads via POST, and it breaks the read=GET assumption baked into `verify` and idempotency"
---

# Ship the Dropbox connector

## Goal

Add Dropbox to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**Reads that are POSTs.** Dropbox's API v2 is RPC: `POST /2/files/list_folder` with a JSON body, even to read. A `verify` operation is defined as a read; here the read is a POST.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <access_token>`.

**Curated operation set (a starting point, not a mandate):** list a folder, get file metadata, search files, create a folder, get a temporary link

## Hazards specific to this one

State plainly whether a POST-shaped read can be a `verify` operation — the configuration contract says a `verify` op *is a read* and runs unattended, and idempotency is declared per operation, so a POST that is idempotent is expressible but unusual. Content upload/download uses a different host and a `Dropbox-API-Arg` **header carrying JSON**; exclude it and say so.

## Acceptance

- [x] `providers/dropbox.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/dropbox.toml`, 6
      operations (`dropbox-user-me`, `dropbox-folder-list`, `dropbox-metadata-get`, `dropbox-search`,
      `dropbox-folder-create`, `dropbox-temporary-link-get`).
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every `[[operations]]` block in
      `providers/dropbox.toml` carries `risk = "medium"` and `idempotency = "non_idempotent"` (the only
      honest pair a POST-only vendor can declare — see the header comment's two numbered questions) and
      a `description` written for a tool-calling model.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → `providers/dropbox.toml`'s single `[[config]]` block (`access_token`), asserted by
      `the_access_token_is_configurable_and_carries_no_example_value`.
- [x] A `verify` operation that is a read and runs unattended. → `verify = "dropbox-user-me"`, a POST
      with no side effects; the loader accepts it because `validate_verify` checks declared `risk`, not
      method (`crates/connector-spec/src/provider.rs:664-687`). Asserted by
      `the_verify_operation_is_a_post_read_declared_medium_risk`.
- [x] `crates/connector-flux/tests/dropbox_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. → 10 tests;
      the two archetype-defining ones are `every_operation_is_a_post_including_the_reads` and
      `no_dropbox_operation_declares_itself_idempotent`.
- [x] **Failing-first test:** the contract test must fail before `providers/dropbox.toml` exists. → all
      10 tests failed at the merge base (`179b7c536bf4a7a4a84b010393d540b1085a6dc2`) on
      "cannot read providers/dropbox.toml"; all 10 pass now.
- [x] The scoped gate is green: `build --provider dropbox`, `diff --provider dropbox` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. → all green;
      see the implementor report for the eight (plus one) expected exceptions below.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly the eight
      tabulated tests are red, plus the coordinator-owned ninth
      (`the_recorded_floor_is_the_measured_figure`, coverage 122/140 vs. floor 105) — reported, neither
      silenced nor fixed.

## Notes

- **Charter fit.** Dropbox is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/dropbox.rs` is **not** in that set and is yours to commit.

## Progress

Shipped a 6-operation connector (`dropbox-user-me` as `verify`, `dropbox-folder-list`,
`dropbox-metadata-get`, `dropbox-search`, `dropbox-folder-create`, `dropbox-temporary-link-get`),
sticking to the story's suggested starting set plus the connection check. Paths, methods, host and
auth scheme are all well-established, long-stable Dropbox API v2 facts I am confident of (the `/2`
RPC host, the `""`-not-`"/"` root path sentinel, the `error_summary`/`error` non-2xx envelope, the
separate `content.dropboxapi.com` host with its `Dropbox-API-Arg` header for upload/download).

Two things I could not verify against a live account and am naming rather than guessing silently
past:

- **`dropbox-user-me`'s exact wire-level request when it carries no body.** `get_current_account`
  takes no arguments, and this pipeline's emitter sends no `body` field and no `Content-Type` header
  at all when an operation declares zero body params (`crates/connector-flux/src/op.rs`,
  `has_body = !body_params.is_empty() || free_form.is_some()`). I believe Dropbox accepts an
  argument-less POST with no body for a no-arg route, but I have not exercised this against Dropbox's
  actual servers and cannot from this repository (no network access is a stated invariant). If it
  turns out Dropbox wants an explicit literal `null` body or a `Content-Type: application/json`
  header even with an empty body, that is a real gap this connector would carry silently until
  someone runs it live.
- **`dropbox-search`'s exact response shape for `matches[].metadata`.** Dropbox's `search_v2` wraps
  each match's metadata in a nested tagged union (`{".tag": "metadata", "metadata": {...}}`) that I
  am less than fully confident of at the byte level, so `operations.response_schema` describes
  `matches` generically (`type = "object"` per entry) rather than modelling that nesting — the same
  choice `providers/box.toml` made for fields it was less sure of than the top-level shape.

Everything else in `providers/dropbox.toml` I shipped without a qualifying note above because I am
confident in it from long-stable, well-documented Dropbox API v2 behaviour, not from repository
convention alone.
