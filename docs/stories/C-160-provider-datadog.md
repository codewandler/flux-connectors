---
id: C-160
title: Ship the Datadog connector
pillar: Spec
status: in-progress
priority: 2
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: needs TWO credentials on one request (DD-API-KEY + DD-APPLICATION-KEY). Every shipped provider sends exactly one. If `[[auth]]` is single-valued this is a refusal story, and that is a finding worth having"
---

# Ship the Datadog connector

## Goal

Add Datadog to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**Two credentials on one request.** Datadog authenticates with `DD-API-KEY` *and* `DD-APPLICATION-KEY` headers together; read operations need both. No shipped provider sends more than one credential, so this is the first test of whether the auth model is plural at all.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** Two custom headers, both secret: `DD-API-KEY` and `DD-APPLICATION-KEY`.

**Curated operation set (a starting point, not a mandate):** list monitors, get a monitor, submit an event, list incidents, query metrics over a time range

## Hazards specific to this one

If the schema admits only one `[[auth]]`, **do not fake it by putting the second key in `params.header`** — that ships a secret as a caller-supplied argument, which is the exact spelling C-55 exists to refuse. Record the refusal with the `path:line` that proves it and stop.

## Acceptance

- [x] `providers/datadog.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/datadog.toml`, 4
      operations (monitor list/get, incident list/get).
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every `[[operations]]` block in
      `providers/datadog.toml`; `effects` is derived (`crates/connector-flux/src/op.rs:616`), never
      authored.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → `providers/datadog.toml`'s two `[[config]]` blocks (`api_key`, `application_key`).
- [x] A `verify` operation that is a read and runs unattended. → `verify = "datadog-monitor-list"`,
      `risk = "low"`, no parameters.
- [x] `crates/connector-flux/tests/datadog_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. → asserts
      `default_auth` is one AND-mechanism naming both credentials, every operation's `effective_auth`
      resolves to it with `requirement.len() == 2`, and the rendered `credential_mechanisms` shape is
      one outer alternative of two, not two of one.
- [x] **Failing-first test:** the contract test must fail before `providers/datadog.toml` exists. → see
      `BASE_PROOF` in the handoff report; verified directly in this worktree by moving the TOML aside
      and rerunning the suite before restoring it.
- [x] The scoped gate is green: `build --provider datadog`, `diff --provider datadog` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly the eight
      `AGENTS.md` names, across the same five binaries; the ninth (`response_schema_coverage`) stayed
      green (see `## Progress`).

## Notes

- **Charter fit.** Datadog is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/datadog.rs` is **not** in that set and is yours to commit.

## Progress

**The finding: this is not a refusal story. Two credentials on one request were already
expressible** — `Operation::auth`/`Connector::default_auth` (`Vec<AuthRequirement>`) is an OR of
mechanisms, and each `AuthRequirement` is itself an AND-set of credentials
(`crates/connector-spec/src/auth.rs:272-288`; `crates/connector-spec/src/ir.rs:652-668`). The
emitted `credentials: &[&[...]]` shape in `crates/catalog/src/generated/postmark.rs` is that
two-level shape rendered flat: outer = OR, inner = AND
(`crates/connector-cli/src/catalog.rs:320-336`, whose own doc comment already names the worked
example — babelforce's `accessId`/`accessToken` AND-pair — as the motivating case). What was new is
that **no shipped connector had ever put a two-credential `AuthRequirement` on a real operation**:
`providers/babelforce.toml` documents that exact pair and deliberately excludes it (vendor
deprecation), so the mechanism existed and was IR-tested (`crates/connector-spec/tests/ir_roundtrip.rs:22-23,236-237`)
but genuinely unshipped. `providers/datadog.toml` ships
`default_auth = [{ credentials = ["datadog.api_key", "datadog.application_key"] }]` — one
alternative, two credentials — and after `build --provider datadog`,
`crates/catalog/src/generated/datadog.rs:42,52,62,72` confirms it renders as
`credentials: &[&["datadog.api_key", "datadog.application_key"]]`, not flattened into two
alternatives. **This is the answer for C-164 (Algolia), which was waiting on it**: no
`connector-spec` change is needed to ship a two-credentials-on-one-request connector; the shape
already carries it.

**Curated to 4 operations, not the story's suggested 5 — two were dropped for want of live
verification, not for the auth question.** `list monitors`, `get a monitor`, `list incidents`,
`get an incident` ship. Dropped:

- **Submit an event.** Datadog documents both `POST /api/v1/events` and a newer
  `POST /api/v2/events`; a `WebFetch` against `docs.datadoghq.com/api/latest/events/` today
  confirmed both paths exist but returned only the page's navigation, not field-level request/response
  detail (the reference tables render client-side). Guessing the body shape — and which envelope is
  current — is exactly the invented output this repository refuses. **Unverified; left out.**
- **Query metrics over a time range.** Datadog's metrics query syntax (`avg:system.load.1{*}`)
  carries `{`, `}`, `:`, `*`; this pipeline's query parameters are not percent-encoded
  (`AGENTS.md` Intentional gaps; `providers/sentry.toml` and `providers/fly.toml` already exclude
  free-text or even safe-looking query filters for the same reason). **Left out for the query-
  encoding gap, not for lack of verification.**

**Endpoint paths for the four shipped operations were checked, not assumed.** `GET /api/v1/monitor`,
`GET /api/v1/monitor/{monitor_id}`, `GET /api/v2/incidents`, `GET /api/v2/incidents/{incident_id}`
were confirmed live via `WebFetch` against `docs.datadoghq.com` on 2026-07-31. The Monitors v1 object
shape (`id`, `name`, `type`, `query`, `message`, `tags`, `options`, `overall_state`, `created`,
`modified`, `creator`, `multi`) is declared from this pipeline's established knowledge of that
long-stable public shape, not from a page fetch (the same fetch that confirmed the incident paths
returned no field-level schema for either resource). **The Incident Management v2 `attributes` shape
is genuinely unverified this session** — its docs page renders field tables client-side, and this
repository's Incident Management schema has changed more than once (a custom "fields" system replaced
fixed severity/state attributes). Rather than guess, `datadog-incident-list`/`datadog-incident-get`
carry no `response_schema` at all, the same honest-absence choice `providers/babelforce.toml` makes
for all nine of its operations. This is worth a follow-up: an agent with a way to read Datadog's
actual OpenAPI document (vendored, or a fetch that survives the client-rendered docs) could turn this
into a verified schema without changing anything else about the connector.

**Gate, measured:** `build --provider datadog` and `diff --provider datadog` are clean (`7 artifacts
up to date`). `cargo test --workspace --no-fail-fast` reports exactly the eight red tests
`AGENTS.md` tabulates, across the same five binaries, and nothing else — the ninth
(`response_schema_coverage`'s three tests) stayed green, because two of the four shipped operations
carry a real, non-permissive schema and the catalogue-wide slack easily absorbs two operations added
without one.
