---
id: C-166
title: Ship the GitLab connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "a project is addressed as a URL-ENCODED path (`group%2Fsub%2Fproject`), so a path segment must survive percent-encoding — the same gap as zendesk-ticket-search, in the path position"
---

# Ship the GitLab connector

## Goal

Add GitLab to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A percent-encoded path segment.** GitLab addresses a project either by numeric id or by `namespace%2Fproject`. The encoded form is what humans have, and `AGENTS.md` records that this pipeline does not encode values.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <pat>`.

**Curated operation set (a starting point, not a mandate):** list issues, get an issue, create an issue, list merge requests, get a pipeline, list a project's branches

## Hazards specific to this one

The honest move may be to accept **only the numeric project id** and document that the path form is unavailable until encoding lands — that is the selection rule C-106 used for Stripe. Say which you chose. Do not hand a model a parameter it must pre-encode itself.

## Acceptance

- [x] `providers/gitlab.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/gitlab.toml`, 7
      operations (the six the story names plus the parameterless `verify` read).
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every `[[operations]]` block in
      `providers/gitlab.toml`; `effects ["network"]` is emitted automatically by
      `crates/connector-flux/src/op.rs` for every HTTP operation and needs no author input.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → `providers/gitlab.toml`'s one `[[config]]` field (the PAT), asserted by
      `gitlab_connector.rs::the_config_surface_asks_for_the_token_and_nothing_else`.
- [x] A `verify` operation that is a read and runs unattended. → `verify = "gitlab-user-get"`,
      `GET /user`, `risk = "low"`, no parameters.
- [x] `crates/connector-flux/tests/gitlab_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. →
      `every_project_reference_is_a_bare_numeric_id` and
      `no_operation_declares_a_project_path_parameter` are the load-bearing pair; the rest are the
      standard per-provider shape checks.
- [x] **Failing-first test:** the contract test must fail before `providers/gitlab.toml` exists. → see
      `BASE_PROOF` in the implementor's report; all 6 tests failed with "cannot read
      providers/gitlab.toml" before the file existed.
- [x] The scoped gate is green: `build --provider gitlab`, `diff --provider gitlab` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly eight, and
      they are the same eight names AGENTS.md tabulates (see Progress).

## Notes

- **Charter fit.** GitLab is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/gitlab.rs` is **not** in that set and is yours to commit.

## Progress

**Decision: numeric project id only — the connector ships, it is not a recorded refusal.**

GitLab's `:id` path segment accepts either a numeric project id or a URL-encoded namespaced path
(`group%2Fproject`). This pipeline interpolates every path and query value verbatim
(`crates/connector-flux/src/op.rs`, "nothing percent-encodes them") and registers no encoder, so the
encoded form cannot be accepted without handing a model a parameter it must pre-encode itself — the
thing the story forbids. The selection rule C-106 used for Stripe applies directly: every
project-scoped parameter in `providers/gitlab.toml` is typed as a JSON Schema `integer`, which cannot
carry a `/` or a `%` at all — safe by construction, not merely untested. The cost is stated on every
such parameter's `description`: a caller who has only `group/project` cannot use this connector until
a percent-encoder lands. Unlike Notion's C-107 first attempt, nothing here forces the whole connector
to be withheld — the numeric-id branch covers the six operations the story names cleanly, so the
"successful outcome" here is a shipped, honestly-scoped connector rather than a refusal.

Shipped as `providers/gitlab.toml` (7 operations: the six the story names plus the parameterless
`gitlab-user-get` verify read) with `crates/connector-flux/tests/gitlab_connector.rs` (6 tests). Scoped
gate green: `build --provider gitlab` writes 10 artifacts, `diff --provider gitlab` reports no drift,
`cargo build --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` are clean, and
`cargo fmt --all --check` is clean. `cargo test --workspace --no-fail-fast` leaves exactly eight tests
red, across the same five binaries and with the same names AGENTS.md's table lists:
`the_provider_list_matches_the_repository`, `the_catalog_is_not_empty` (catalog::embedded_operations);
`the_committed_tree_is_a_fixed_point_of_a_build` (connector-cli::catalog_artifacts);
`a_build_plans_both_readme_images_and_they_are_current` (connector-cli::readme_snippet);
`the_shipped_artifacts_are_byte_identical`, `the_published_catalogue_carries_the_service`
(connector-cli::service_units); `every_shipped_operation_carries_its_metadata_and_its_flux`,
`the_build_writes_and_checks_site_catalog_json` (connector-cli::site_catalog). The count matches; not a
finding.

**Unverified / not shipped, named rather than guessed:**

- **Self-managed GitLab is out of scope.** `base_url` is the fixed `https://gitlab.com/api/v4`, the
  same choice `providers/sentry.toml` makes for `sentry.io`. A self-managed instance lives at an
  operator-chosen host with no binding this pipeline can express yet (C-68).
- **No free-text `search` filter on any list operation**, even though the real GitLab API offers one
  on issues, merge requests and branches. It is exactly the shape `zendesk-ticket-search` already
  demonstrates cannot survive verbatim interpolation. Only a closed `state` enum and integer
  pagination (`page`/`per_page`) are declared, both safe by construction.
- **Response schemas are authored from `docs.gitlab.com/ee/api/*` reference knowledge, not a vendored
  spec** — GitLab publishes no single canonical machine-readable OpenAPI document for its whole REST
  API the way Stripe or GitHub do, so provenance is hand-authored and drift is undetectable by
  machine, the same caveat zendesk, freshdesk, github and notion already carry. Field sets are
  deliberately partial (the identity, the state, and what a flow branches on), not a claim of
  exhaustiveness, matching every other shipped provider's convention.
- **`gitlab-issue-create`'s `labels` field is declared as a comma-separated string**, not an array,
  because that is the shape GitLab's issue-create endpoint documents; this is stated in the field's
  `description` rather than left implicit, since it is the one place this connector's body shape
  departs from GitHub's array convention.
