---
id: C-181
title: Ship the Statuspage connector
pillar: Spec
status: done
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "`Authorization: OAuth <token>` — a fourth Authorization scheme word, and a page id prefixes every path"
---

# Ship the Statuspage connector

## Goal

Add Statuspage to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**`OAuth` as a literal scheme word.** Atlassian Statuspage sends `Authorization: OAuth <key>`, which is neither bearer nor basic nor OAuth2's own bearer usage.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: OAuth <api_key>`.

**Curated operation set (a starting point, not a mandate):** list incidents, get an incident, create an incident, update an incident, list components

## Hazards specific to this one

Fourth in the Authorization-shape family with [C-161](C-161-provider-okta.md), [C-162](C-162-provider-pagerduty.md), [C-175](C-175-provider-launchdarkly.md). Creating a Statuspage incident is **publicly visible immediately** — declare that effect as external-facing, not as an ordinary create.

## Acceptance

- [x] `providers/statuspage.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents.
      → five operations, the five this story names. `providers/statuspage.toml`; the exclusions and
      why each was excluded are in the file's own header comment.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
      → `risk`/`idempotency` on all five. **`effects` is not authorable** — see `## Progress`.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → `providers/statuspage.toml`'s two `[[config]]` blocks;
      `statuspage_connector.rs::the_page_id_folds_into_the_base_url_as_one_bound_variable`.
- [x] A `verify` operation that is a read and runs unattended.
      → `verify = "statuspage-component-list"`;
      `statuspage_connector.rs::verify_is_an_unattended_read_that_needs_no_argument`.
- [x] `crates/connector-flux/tests/statuspage_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses.
      → 8 tests. The archetype one is
      `the_scheme_word_oauth_is_a_prefix_and_not_an_oauth2_grant`, which asserts
      `AuthScheme::Header { name: "Authorization", prefix: "OAuth " }` **and** `oauth2.is_none()`.
- [x] **Failing-first test:** the contract test must fail before `providers/statuspage.toml` exists.
      → all 8 failed at merge base `3457581` with
      `cannot read …/providers/statuspage.toml: No such file or directory (os error 2)`.
- [x] The scoped gate is green: `build --provider statuspage`, `diff --provider statuspage` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
      → `diff` reports `8 artifacts up to date (1 provider checked)`; clippy and fmt clean.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding.
      → exactly eight, across exactly the five binaries `AGENTS.md` names, and no others. The ninth
      (`the_recorded_floor_is_the_measured_figure`) is **green** — see `## Progress`.

## Notes

- **Charter fit.** Statuspage is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/statuspage.rs` is **not** in that set and is yours to commit.

## Progress

**Shipped, not refused.** The story allowed for a recorded refusal; C-184's prefix axis made one
unnecessary. Five operations, two config fields, one credential.

### The question it was chosen for: `OAuth` is a scheme word

`scheme = { header = { name = "Authorization", prefix = "OAuth " } }`, and **no `[auth.oauth2]`
block**. The word in the header invites one and would be wrong: the key is a static string a human
pastes in, and declaring an `oauth2` block would tell the host to run an effectful grant against an
authorization server that does not exist for it.
`crates/connector-flux/tests/statuspage_connector.rs::the_scheme_word_oauth_is_a_prefix_and_not_an_oauth2_grant`
asserts `oauth2.is_none()` so the trap stays pinned for the next author who reads `OAuth` and
reaches for the block.

The trailing space is load-bearing: the host concatenates prefix and secret with nothing between,
so `"OAuth"` would travel as `OAuth<key>`. Since `3457581` that is a **load error** rather than a
silent 401 — `crates/connector-spec/src/provider.rs::validate_auth_prefix` refuses a non-empty
prefix ending in an alphanumeric.

**Statuspage is the first shipped connector carrying a non-empty prefix.** Every `header` credential
before it omits the field (figma, gitlab, shopify, launchdarkly), so
`crates/catalog/src/generated/statuspage.rs:24` —
`crate::Placement::Header { name: "Authorization", prefix: "OAuth " }` — is the first committed
artifact where C-184's axis carries a value.

### The page id folds into `base_url` — and this is *not* the C-187 gap

`base_url = "https://api.statuspage.io/v1/pages/{page_id}"`, bound by one `[[config]]` field
(`binds = "endpoint.page_id"`). This needed no new mechanism: `providers/docusign.toml:117` already
ships a `base_url` path placeholder pinned by `binds = "endpoint.account_id"`, and Statuspage's page
id is identical in shape — a *prefix* of every path.

**The real cost, recorded rather than worked around:**

- `GET /v1/pages` (list administrable pages) and `GET /v1/pages/{page_id}` (read one page's
  settings) sit *above* this base URL and are **unreachable** from this connector. DocuSign hit
  exactly this at its own `/accounts/{account_id}` tail and chose a sibling read for `verify`; this
  connector does the same, with `/components`.
- An account administering several pages needs **one installation per page**, because the page is
  connection configuration and not a per-call argument.

**C-187 remains the right story for the general problem.** What makes the fold work here is only
that the id happens to sit at the front of every path. Had it sat mid-path or in a query string —
Cloudflare's `zone_id`, Vercel's `teamId` — no spelling would exist and this story would have been a
refusal.

### "Publicly visible" is not expressible, and this connector does not imply it is

The story asks for the create to be declared "as external-facing, not as an ordinary create". **No
field can say that, and none was invented:**

- `effects` is not authorable at all. `crates/connector-flux/src/op.rs:616` hardcodes
  `effects: vec![from_tag("network")?]` on every generated op — the measurement C-155 recorded.
- `Risk` has four values (`crates/connector-spec/src/ir.rs:85-94`): `low`, `medium`, `high`,
  `destructive`. None means external-facing.

So both writes are `risk = "high"`, matching `github-issue-create` and `launchdarkly-flag-toggle`.
The asymmetry the scale cannot carry is stated in each operation's own `description`, which is the
one string a model actually reads: **the incident is reversible — resolve or delete it — the
subscriber email and SMS are not.** `medium` would be a lie about the audience, `destructive` a lie
about the reversibility. `statuspage_connector.rs::the_public_writes_are_high_risk_and_the_scale_cannot_say_why`
asserts the `high`, asserts the descriptions name the public page and the subscribers, and asserts
the provider file declares no `effects` key.

`deliver_notifications` is a **required** body field on both writes. Under C-56 an optional body
field travels as an explicit `null`, so every declared body field had to be required anyway —
requiring this one is free in mechanism and buys the only thing available: a caller cannot post to a
public status page without stating, in the call itself, whether every subscriber gets an email and a
text about it. That explicit choice is the closest this model gets to declaring the effect it cannot
name.

### Left out as unverified, not guessed at

- **Pagination.** Statuspage's incident and component collections take `page`/`per_page`. No
  verified account of their bounds or defaults, so no `minimum`/`maximum` was invented. Both
  collection reads return the vendor's own first page. This is the honest gap: a page with a long
  incident history is not enumerated exhaustively here.
- **The incident search `q`** — a free-text query value, exactly the C-30 defect. **No operation in
  this connector declares a query parameter at all**
  (`statuspage_connector.rs::no_operation_reaches_a_query_string_at_all`), which makes C-30 harmless
  here rather than merely unlikely.
- **`impact_override`** — under C-56 it would become mandatory on every create, permanently
  replacing Statuspage's own impact calculation.
- **`component_ids` / `components`** — same C-56 problem, plus a dynamic-key object shape this file
  cannot verify. An incident naming no component is valid on Statuspage.
- **Incident delete, the component `PATCH`, scheduled maintenances**, the `scheduled`/`in_progress`/
  `verifying`/`completed` statuses, and `postmortem` — none selected by this story, and the last
  would let a caller move an incident into a state this connector cannot then populate.
- **`name` on the update.** Deliberately not declared: under C-56 it would be required, and a caller
  restating a title on every update is a caller who can silently rename a live public incident by
  mistyping it.

### Gate

- `build --provider statuspage` → 8 artifacts written; `diff --provider statuspage` →
  `8 artifacts up to date (1 provider checked)`.
- `cargo test --workspace --no-fail-fast` → **exactly eight red**, across exactly the five binaries
  `AGENTS.md` tabulates and no others. All eight are whole-catalogue staleness checks, red because
  this story correctly did not write a coordinator-owned artifact.
- **The ninth check is green.** `the_recorded_floor_is_the_measured_figure` passes: coverage is
  210/237 (88%) against `COVERED_FLOOR = 193`, so this story's five operations — all five carrying a
  response shape — fit inside the two-way ratchet's slack on their own. `COVERED_FLOOR` was **not**
  touched; per `AGENTS.md` the coordinator raises it at integration if the wave's accumulation
  crosses the slack.
- Clippy and `cargo fmt --all --check` clean.

### Not touched

`CHANGELOG.md`, `docs/stories/README.md`, `docs/roadmap.md`, `Cargo.lock`, `Cargo.toml`,
`crates/catalog/src/generated.rs`, `web/public/catalog.json`, `web/public/v1/**`,
`assets/readme-snippet-*.svg`, `COVERED_FLOOR`. `status` left at `in-progress` for the coordinator
to close.
