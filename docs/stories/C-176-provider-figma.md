---
id: C-176
title: Ship the Figma connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "`X-Figma-Token` — a custom header credential, and a file is addressed by a key taken from a URL a human copied"
---

# Ship the Figma connector

## Goal

Add Figma to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A custom header credential on a read-mostly API.** Shopify's `header` scheme exists; Figma is the case where the whole API is read-only, so every operation is idempotent and the risk declarations should reflect that uniformly.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `X-Figma-Token: <token>`.

**Curated operation set (a starting point, not a mandate):** get a file, get file nodes, get image renders, list project files, list comments

## Hazards specific to this one

If every operation is a read, say so and let the declarations be uniform — a fleet where one connector is honestly all-idempotent is useful evidence for the tool-contract surface. `get image renders` is a POST-free read that returns URLs with an expiry; note the expiry rather than implying the URLs are stable.

## Acceptance

- [x] `providers/figma.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/figma.toml`: 6
      operations (`figma-user-me`, `figma-file-get`, `figma-file-nodes-get`,
      `figma-image-render-get`, `figma-project-files-list`, `figma-file-comments-list`).
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every operation declares `risk = "low"` /
      `idempotency = "idempotent"`; `effects` is derived by the emitter (`["network"]`), asserted in
      `every_figma_operation_emits_a_module_that_parses_analyzes_and_is_canonical`.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → `providers/figma.toml`'s single `[[config]]` field (`token`, `binds = "credential.figma.token"`,
      `secret = true`); the loader itself enforces the secret/binds agreement (`provider.rs`'s
      `Binding::is_secret`), and `cargo run -p connector-cli -- build --provider figma` passing is
      that check exercised.
- [x] A `verify` operation that is a read and runs unattended. → `verify = "figma-user-me"`
      (`GET /v1/me`, `risk = "low"`), asserted in `the_connector_verifies_with_the_current_user_read`.
- [x] `crates/connector-flux/tests/figma_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. →
      `every_operation_is_a_read_declared_uniformly_low_risk_and_idempotent` (the uniformity claim),
      `the_image_render_operation_notes_the_url_expiry_rather_than_implying_stability` (the expiry
      hazard), `the_declared_query_parameter_is_restricted_to_a_safe_charset` (the query-encoding
      safety argument).
- [x] **Failing-first test:** the contract test must fail before `providers/figma.toml` exists. → see
      `BASE_PROOF` in the implementation report; all 7 tests failed on a missing-file panic at the
      merge base.
- [x] The scoped gate is green: `build --provider figma`, `diff --provider figma` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. → all green
      except the eight expected whole-catalogue staleness tests below.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly eight, matching
      `AGENTS.md`'s table by name: `the_provider_list_matches_the_repository`,
      `the_catalog_is_not_empty`, `the_committed_tree_is_a_fixed_point_of_a_build`,
      `a_build_plans_both_readme_images_and_they_are_current`,
      `the_shipped_artifacts_are_byte_identical`, `the_published_catalogue_carries_the_service`,
      `every_shipped_operation_carries_its_metadata_and_its_flux`,
      `the_build_writes_and_checks_site_catalog_json`.

## Progress

- Six operations shipped, all `GET`, all `risk = "low"` / `idempotency = "idempotent"` — Figma's
  curated REST surface has no write endpoint, so the uniformity is the honest answer rather than an
  invented pattern (see `providers/figma.toml`'s header comment and
  `every_operation_is_a_read_declared_uniformly_low_risk_and_idempotent`).
- **Unverified against a live account, named rather than silently assumed:**
  - The exact shape of `GET /v1/me`'s response (`id`/`email`/`handle`/`img_url`) and whether `id` is
    numeric or an opaque string — declared as `string` to be safe.
  - The precise error envelope (`{"status": ..., "err": "..."}`) — declared with
    `message_pointer = "/err"` from documentation memory, not a live 4xx response.
  - Whether Figma node ids are always strictly `<uint>:<uint>` with no other separator in every
    account/plan — the `ids` query parameter's `pattern` assumes this; if a node id ever carries a
    different shape, both the pattern and `the_declared_query_parameter_is_restricted_to_a_safe_charset`
    need revisiting together.
  - The exact expiry window for `figma-image-render-get`'s rendered-image URLs is **not** stated as a
    number of hours/days anywhere in the connector — only that the URLs are temporary and should not
    be persisted. This is deliberate: a specific duration could not be verified with confidence, and a
    wrong number would be exactly the plausible-but-incorrect output this pipeline refuses. If a
    verified figure becomes available, it belongs in both the operation's `description` and its
    `response_schema`.
- No operation was left out for a documented, checkable reason the way `providers/shopify.toml`
  excludes query-parameterised collection endpoints — the two operations that do need a query
  parameter here (`figma-file-nodes-get`, `figma-image-render-get`) are included because their `ids`
  values are provably restricted to a safe charset (digits, `:`, `,`), unlike Shopify's free-text
  filters.
- Optional Figma query parameters (`depth`, `geometry`, `version` on the file endpoints; `format`,
  `scale` on the image-render endpoint) are deliberately not declared — curation, not an oversight to
  backfill.

## Notes

- **Charter fit.** Figma is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/figma.rs` is **not** in that set and is yours to commit.
