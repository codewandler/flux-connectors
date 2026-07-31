---
id: C-170
title: Ship the Vercel connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "team scope is an OPTIONAL query parameter (`?teamId=`) that changes which account the write lands on — an optional argument with a blast radius"
---

# Ship the Vercel connector

## Goal

Add Vercel to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**An optional parameter that changes the target account.** Vercel takes `?teamId=`; omit it and the call applies to the personal account instead. It is optional in the API and load-bearing in effect.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <token>`.

**Curated operation set (a starting point, not a mandate):** list projects, get a project, list deployments, get a deployment, cancel a deployment

## Hazards specific to this one

An optional parameter whose absence silently redirects a write is worth a `description` that says so — the configuration contract notes every `description` is text a *model* reads. Compare Fly.io ([C-111](C-111-provider-fly.md)), the other deployment provider, and do not duplicate what it already covers.

## Acceptance

- [x] `providers/vercel.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. Five operations: list projects,
      get a project, list deployments, get a deployment, cancel a deployment.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. See `providers/vercel.toml`.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      One field (`token`, `binds = "credential.vercel.token"`); `teamId` cannot be a `[[config]]`
      field at all — see `## Progress`.
- [x] A `verify` operation that is a read and runs unattended. `verify = "vercel-projects-list"`, a GET.
- [x] `crates/connector-flux/tests/vercel_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. See
      `every_operation_declares_team_id_and_names_the_personal_account_fallback` and
      `list_operations_name_the_hazard_in_their_own_description`.
- [x] **Failing-first test:** the contract test must fail before `providers/vercel.toml` exists. See
      `BASE_PROOF` in the handoff report — all 8 tests failed with "cannot read providers/vercel.toml"
      before the file existed.
- [x] The scoped gate is green: `build --provider vercel`, `diff --provider vercel` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. Measured: exactly the
      eight `AGENTS.md` names, across the same five binaries (`catalog::embedded_operations`,
      `connector-cli::catalog_artifacts`, `connector-cli::readme_snippet`,
      `connector-cli::service_units`, `connector-cli::site_catalog`). The ninth,
      `the_recorded_floor_is_the_measured_figure`, stayed **green** — this story's 4 response-schema'd
      operations fit inside the recorded floor's slack alone.

## Notes

- **Charter fit.** Vercel is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/vercel.rs` is **not** in that set and is yours to commit.

## Progress

- **Every path, method and query parameter was checked against Vercel's own reference pages**
  (`vercel.com/docs/rest-api/{projects,deployments}` and the two operation pages for each of the five
  ids below), fetched 2026-07-31, rather than recalled — `GET /v10/projects`, `GET
  /v9/projects/{idOrName}`, `GET /v7/deployments`, `GET /v13/deployments/{idOrUrl}`, `PATCH
  /v12/deployments/{id}/cancel`. Nothing in this file is a guessed path.
- **`teamId` cannot be a `[[config]]` field.** `ConfigField::binds` closes over `endpoint.<var>`,
  `credential.<name>`, `username.<name>`, `oauth.client_id` and `oauth.client_secret`
  (`crates/connector-spec/src/config.rs::parse_binding`) — none of those forms names a per-request
  query parameter, so there is no honest `binds` value to give it. It stays a caller-supplied argument
  on every operation instead of a pre-configured value. This is a real schema gap, not an oversight:
  a future `binds = "query.<name>"` form (or similar) would let an operator pin a default team
  per-connection the way `endpoint.subdomain` pins a tenant host today, and would remove the model's
  chance to omit it. Recorded here rather than worked around.
- **`vercel-projects-list`'s response carries no `response_schema`, deliberately.** Vercel's own
  reference page documents three mutually incompatible top-level shapes for the same `200` (a bare
  array, and two `{pagination, projects}` variants differing on which per-project field is present)
  with nothing to say which an account actually gets — asserting one would be a guess wearing a
  schema's clothes. The other four operations do carry a `response_schema`, each restricted to the
  fields common across every documented variant.
- **Response-shape confidence, named per field.** `vercel-deployments-list`'s schema is the one I have
  full confidence in — Vercel's list-deployments reference page shows a single, unambiguous shape with
  no `oneOf`. `vercel-project-get`, `vercel-deployment-get` and `vercel-deployment-cancel` all union
  multiple documented variants; only fields present in **every** variant are marked `required`, and
  the properties beyond those (`url`, `name`, `target`) are declared but not required, because at
  least one variant omits each of them.
- **`risk = "high"` on `vercel-deployment-cancel`, not `"destructive"`.** Cancelling stops an in-flight
  build; it does not delete a resource the way `fly-machine-delete` does. Classified alongside Fly's
  `stop`/`restart` (interrupts an in-progress or running process) rather than its `delete`.
- **Not verified, and therefore not shipped:** Vercel's error envelope shape (no `quirks.error_envelope`
  declared — I could not confirm one confidently from the fetched pages in the time available), and
  every operation beyond the curated five (domains, env vars, transfers, rollback, OIDC tokens,
  pause/unpause, and the rest of Vercel's REST reference). None of these are guessed at; they are
  simply absent.
