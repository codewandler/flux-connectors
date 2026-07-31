---
id: C-175
title: Ship the LaunchDarkly connector
pillar: Spec
status: in-progress
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "`Authorization: <token>` with NO scheme word at all — tests whether 'no prefix' is expressible distinctly from bearer, or whether it collapses into it"
---

# Ship the LaunchDarkly connector

## Goal

Add LaunchDarkly to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**An Authorization header with no scheme.** LaunchDarkly sends the token raw. `bearer` would send `Bearer <token>`, which LaunchDarkly rejects — so if the model cannot express 'no prefix', this connector cannot work and that is the finding.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: <token>`, raw.

**Curated operation set (a starting point, not a mandate):** list feature flags, get a flag, list environments, list projects, toggle a flag in an environment (a write with immediate production effect)

## Hazards specific to this one

Toggling a flag changes live behaviour for real users: declare that effect honestly rather than as an ordinary update. Related to [C-161](C-161-provider-okta.md) and [C-162](C-162-provider-pagerduty.md) — all three are the same question (can the Authorization value be shaped?) with three different answers wanted. Whichever runs first should record the enum's definition site so the others need not re-measure.

## Acceptance

- [x] `providers/launchdarkly.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. Five operations: list projects,
      list a project's environments, list a project's feature flags, get one feature flag, toggle a
      flag on/off in one environment.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. The four reads are `risk = "low"`; the toggle is
      `risk = "high"` with a description naming the immediate live production effect, and
      `idempotency = "non_idempotent"` (see `## Progress` — not `"idempotent"`, because this
      repository's emitter refuses that on any `PATCH` under RFC 9110 §9.2.2).
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      One field, `api_token` (`secret = true`, `binds = "credential.launchdarkly.api_token"`).
- [x] A `verify` operation that is a read and runs unattended. `verify = "launchdarkly-project-list"`.
- [x] `crates/connector-flux/tests/launchdarkly_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. Asserts (1)
      the raw, unprefixed `Authorization` header round-trips as `AuthScheme::Header { name:
      "Authorization" }` with no second field, (2) no emitted operation carries the literal word
      `Bearer` or `Basic`, (3) the toggle is `risk = "high"` with a description naming the live effect,
      while every other operation stays `risk = "low"`, and (4) the toggle's JSON Patch body schema
      admits only a single `replace` onto one environment's `on` bit.
- [x] **Failing-first test:** the contract test must fail before `providers/launchdarkly.toml` exists.
      See `BASE_PROOF` in the report — at `$(git merge-base main HEAD)` neither the TOML nor the test
      file exist, so `cargo test -p connector-flux --test launchdarkly_connector` errors `no test
      target named launchdarkly_connector`.
- [x] The scoped gate is green: `build --provider launchdarkly`, `diff --provider launchdarkly` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. See `GATE` in
      the report.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. **A ninth was also red**:
      `the_recorded_floor_is_the_measured_figure` (`crates/connector-spec/tests/response_schema_coverage.rs`),
      the wave-level `COVERED_FLOOR` ratchet AGENTS.md names as coordinator-owned as of 2026-07-31 —
      reported below, `COVERED_FLOOR` left untouched. See `## Progress`.

## Progress

- **2026-07-31 — shipped. The auth question resolves to "yes, already, today," exactly as C-161's
  probe on Okta predicted for this story.** `AuthScheme::Header { name: "Authorization" }` expresses
  LaunchDarkly's raw, unprefixed token completely — no change to `connector-spec`, no collision with
  C-184 (which owns the separate prefix axis Okta/PagerDuty/Statuspage are blocked on). Proven by
  `crates/connector-flux/tests/launchdarkly_connector.rs::the_launchdarkly_connector_authenticates_with_a_raw_unprefixed_header`
  (the round trip to exactly `[header]\nname = "Authorization"`) and
  `::no_emitted_operation_carries_a_bearer_or_basic_word` (the emitted Flux carries no scheme word).
- **A second, unanticipated finding: `PATCH` cannot declare `idempotency = "idempotent"` in this
  pipeline, full stop.** `crates/connector-flux/src/lib.rs`'s `WriteDeclaredIdempotent` refuses it
  under RFC 9110 §9.2.2 regardless of what the specific operation actually does on the wire — only
  `PUT` and `DELETE` are left alone. A JSON Patch `replace` onto a boolean is, in isolation,
  idempotent in the everyday sense (repeating it lands in the same state), but the loader does not
  reason about the endpoint's own semantics, only its HTTP method. `launchdarkly-flag-toggle` is
  declared `idempotency = "non_idempotent"` accordingly — the honest answer under this repository's
  rule, not a claim about what LaunchDarkly itself does with a repeated PATCH.
- **Endpoints named with reasonable confidence, not verified against a live account (no network
  access in this pipeline; see `AGENTS.md`'s "no provider can make a live call"):** `GET /projects`,
  `GET /projects/{project_key}/environments`, `GET /flags/{project_key}`, `GET
  /flags/{project_key}/{feature_flag_key}`, and `PATCH /flags/{project_key}/{feature_flag_key}` with a
  JSON Patch (RFC 6902) body, all under `https://app.launchdarkly.com/api/v2`. These are drawn from
  LaunchDarkly's own published API conventions (the `items`/`totalCount` list envelope, the `env`
  query filter restricting a flag's per-environment representation, JSON Patch as the update
  mechanism) rather than from a vendored spec — no `specs/launchdarkly/` exists, so drift is
  undetectable by machine exactly as for zendesk, freshdesk, github, notion, calendly and shopify.
- **Deliberately excluded, named rather than guessed at:** flag-list's `tag` filter (not confident
  enough of its query-value character set to declare a safe `pattern`), flag creation/deletion and
  targeting-rule edits (would need a JSON Patch/semantic-patch surface this curated connector was
  never authored or verified against), and cursor-based pagination (LaunchDarkly's list endpoints page
  with plain `limit`/`offset` integers instead, which carry no encoding hazard).
- **Nine tests red, not eight, and the ninth is coordinator-owned, not a defect in this diff.** The
  eight whole-catalogue staleness checks AGENTS.md tabulates are red for the expected reason (a
  provider implementor correctly did not touch a whole-catalogue artifact); the ninth,
  `the_recorded_floor_is_the_measured_figure`, is the wave-level `COVERED_FLOOR` ratchet in
  `crates/connector-spec/tests/response_schema_coverage.rs`, explicitly fenced to the coordinator by
  this story's dispatch and by AGENTS.md's 2026-07-31 update. `COVERED_FLOOR` was not edited.
- **Board not regenerated** — `docs/stories/README.md` is coordinator-owned; `status` moved
  `ready` -> `in-progress` here, so the board needs a `/track:board` run at integration.

## Notes

- **Charter fit.** LaunchDarkly is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/launchdarkly.rs` is **not** in that set and is yours to commit.
