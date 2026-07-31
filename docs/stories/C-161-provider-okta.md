---
id: C-161
title: Ship the Okta connector
pillar: Spec
status: done
priority: 2
design:
epic: provider-fleet-2
areas: [providers]
note: "Connector shipped on C-184's prefix axis — `providers/okta.toml` spells `scheme = { header = { name = \"Authorization\", prefix = \"SSWS \" } }`, five curated operations, one destructive write. The probe's findings stand as the record of why the axis exists; the probe test is now a per-provider contract test"
---

# Ship the Okta connector

## Goal

Add Okta to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A third Authorization scheme.** Okta uses `Authorization: SSWS <apiToken>`. Fifteen providers are `bearer` and two are `basic`; nothing has yet asked for an arbitrary prefix.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: SSWS <token>` — a custom scheme word, not bearer.

**Curated operation set (a starting point, not a mandate):** list users, get a user, list groups, list a user's groups, deactivate a user (destructive, non-idempotent)

## Hazards specific to this one

Read the `scheme` field's accepted values before designing. If it is closed to bearer/basic, say so with the enum's definition site, and say whether the `header` scheme (Shopify's) can carry `SSWS <token>` honestly or whether it would smuggle a prefix into a credential value — a credential value is what this repo must never author.

## Acceptance

> The first attempt (2026-07-31, below) could not tick these and said so. C-184 built the missing
> axis; this pass ships the connector. The annotations record what actually satisfies each item.

- [x] `providers/okta.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. Five operations
      (`providers/okta.toml:105,126,147,168,210`), with the excluded surface — `q`/`filter`/`search`,
      the `after` cursor, every other lifecycle transition, `DELETE /users/{id}`, and a `maximum` on
      `limit` — named and reasoned in the header comment rather than guessed at.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. Four `low`/`idempotent` reads and one
      `destructive`/`non_idempotent` write (`providers/okta.toml:214`). *Effects* are derived, not
      authored — the emitter writes `effects ["network"]` (`connectors/okta.flux`); there is no
      `effects` field in the IR to declare.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      `providers/okta.toml:247` (`domain` → `endpoint.domain`, non-secret) and `:256` (`api_token` →
      `credential.okta.api_token`, `secret = true`, and deliberately no `example`).
- [x] A `verify` operation that is a read and runs unattended. `verify = "okta-user-list"`
      (`providers/okta.toml:82`), a `GET` with no required parameter — asserted by
      `the_user_deactivation_is_the_one_destructive_write_and_the_verify_is_a_read`.
- [x] `crates/connector-flux/tests/okta_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. Rewritten
      from the probe into the shape of `launchdarkly_connector.rs`, loading the shipped TOML. Findings
      1 and 3 are kept verbatim as fixture tests; `no_provider_toml_was_shipped_for_this_probe` is
      **inverted** into `the_shipped_connector_carries_the_ssws_scheme_word_the_probe_could_not`.
- [x] **Failing-first test:** the contract test must fail before `providers/okta.toml` exists. At
      `$(git merge-base main HEAD)` = `3457581`, five of the seven tests fail on the absent file and
      the two kept probe findings pass. See `BASE_PROOF` in the report.
- [x] The scoped gate is green: `build --provider okta`, `diff --provider okta` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. `diff` reports
      `8 artifacts up to date (1 provider checked)`.
- [x] **Exactly eight tests are red and reported, not silenced.** Exactly eight, across the five
      binaries AGENTS.md tabulates, and no ninth — `the_recorded_floor_is_the_measured_figure` is
      green in this worktree. Listed in `## Progress` and in the report's `GATE`.

## Progress

- **2026-07-31 — attempted, blocked at the auth model. Nothing shipped, deliberately.** The probe
  question this story exists to answer — *is an arbitrary Authorization scheme word expressible?* —
  is **no**, and it was measured against the loader rather than read off the design doc:

  1. **`AuthScheme` is a closed, five-member enum.** `crates/connector-spec/src/auth.rs:70-102`:

     ```rust
     #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
     #[serde(rename_all = "snake_case", deny_unknown_fields)]
     pub enum AuthScheme {
         Bearer,
         Basic,
         Header { name: String },
         Query { name: String },
         Signing,
     }
     ```

     A probe fixture declaring `scheme = "ssws"` is refused at deserialization
     (`crates/connector-flux/tests/okta_connector.rs::an_arbitrary_scheme_word_is_not_a_variant_of_auth_scheme`),
     with `toml`'s own `unknown variant` message naming the five it does accept.
  2. **`Header`, the one variant shaped like it could carry a word, has no field to carry it on.**
     `auth.rs:78-82` declares `Header { name: String }` — the header key — and nothing else. A probe
     fixture declaring `scheme = { header = { name = "Authorization", prefix = "SSWS " } }` is
     refused the same way (`deny_unknown_fields`, `unexpected keys in table: prefix`) —
     `the_header_scheme_carries_the_ssws_prefix_it_once_could_not` in the same file.
  3. **`docs/designs/unified-auth.md:75-77` proposed exactly this field** — `prefix` on header
     placement, called "the single highest-value element of this whole design" because it turns
     `Bearer `, `Basic `, `Token ` and `GenieKey ` into one code path — **and it was never
     implemented.** The shipped `AuthScheme::Header` mirrors flux's own four-variant vocabulary
     (`auth.rs:48-58`) exactly, on purpose, so the seam that would carry `SSWS ` does not exist on
     either side today.
  4. **A bare `header` placement aimed at `Authorization` loads — and that is the trap, not the
     answer.** `AuthScheme::Header` does not know or care what header name it is given, so
     `scheme = { header = { name = "Authorization" } }` is legal
     (`a_bare_header_placement_still_omits_the_scheme_word_it_does_not_declare`). But the header's whole
     *value* is the resolved secret and nothing else — the same shape Shopify uses honestly
     (`providers/shopify.toml:166-171`, because `X-Shopify-Access-Token`'s entire value really is the
     secret). Applied to Okta it would emit `Authorization: <token>`, silently missing the literal
     word `SSWS`, which is a request Okta's API rejects. Reaching `Authorization: SSWS <token>` from
     here has exactly one remaining route — baking `"SSWS "` into the credential value the operator
     pastes into `OKTA_API_TOKEN` — and that is the one thing AGENTS.md forbids outright ("no
     credential value enters provider TOML, generated Flux, a manifest, the public catalogue, or the
     lockfile"), so it is refused on sight rather than tried.
  5. **The per-provider compile gate would not have caught this**, the same lesson C-107 recorded
     for Notion: a `header`-scheme Okta connector compiles, formats and round-trips cleanly, and then
     sends a request the vendor answers `401` on every call. Only a test built for this specific
     question — `crates/connector-flux/tests/okta_connector.rs` — asserts it; there is nothing
     generic to add to `shipped_modules.rs` for it, because a wrong-but-well-formed scheme is not a
     shape violation.

- **RESOLVED by [C-184](C-184-auth-scheme-prefix-axis.md) (2026-07-31).** The prefix axis is built.
  `AuthScheme::Header` now carries `{ name, prefix }`, and Okta's scheme word is
  `prefix = "SSWS "` — trailing space included, since the space is part of the literal. The findings
  below stand exactly as measured and are *why* the axis exists; two of the probe tests they cite now
  assert the opposite of what they asserted, which this story's own doc comments said would happen.
  C-184 chose a prefix over a `prefix`+`suffix` pair and over a value template, on the evidence in
  the next bullet — this story had already measured PagerDuty's `Token token=` as "a prefix exactly
  like `SSWS `, just longer", so all three blocked vendors put the credential at the **tail**.
  What C-184 did *not* do is write `providers/okta.toml`; that is still this story's job.

- **What would unblock this, and what it means for the four stories waiting on this answer.**
  Extending `AuthScheme` with a `prefix` field on `Header` (exactly what `unified-auth.md:75-77`
  already proposed and C-19 never finished) is a `connector-spec` change, not a per-provider one — it
  changes the enum every shipped provider's `[[auth]]` deserializes through. Read against the four
  stories named in this story's dispatch:
  - **C-162 (PagerDuty, `Authorization: Token token=<key>`)** is the *same* finding as Okta's: the
    whole value is a fixed literal (`"Token token="`) followed directly by the raw key, which is a
    prefix exactly like `SSWS `, just longer. It is blocked for the identical reason.
  - **C-181 (Statuspage, `Authorization: OAuth <key>`)** is the same finding again — `OAuth ` is a
    literal scheme word, mechanically identical to `SSWS `. Blocked for the identical reason.
  - **C-175 (LaunchDarkly, `Authorization: <token>` raw)** and **C-178 (ClickUp, same)** ask a
    *different* question — whether "no prefix at all" is expressible distinctly from `bearer` — and
    the answer to theirs is **yes, already, today**: `scheme = { header = { name = "Authorization" }
    }` is exactly what `a_bare_header_placement_still_omits_the_scheme_word_it_does_not_declare` proves
    loads and round-trips cleanly, and an empty prefix is precisely what a raw value needs. Neither
    is blocked by this finding.

  So of the four, two (C-162, C-181) need the same enum change this story declines to make, and two
  (C-175, C-178) do not need it at all. This story deliberately does not extend `AuthScheme`: doing
  so is out of a single provider story's fence, it would collide with C-162 and C-181 the moment it
  landed, and the dispatch for this story asked explicitly to record the finding rather than fix it.
  A future story scoped to `connector-spec`'s auth model — reading this note, `unified-auth.md`'s
  `prefix` proposal, and C-162/C-181 together — is the right place to decide whether to add it.
- **No `providers/okta.toml` shipped, and no crate other than the new test file touched.** Shipping
  a `header`-scheme connector would ship a connector that fails closed with `401` on every real call,
  which is the exact failure mode this repository's non-negotiable rules exist to avoid ("a loud
  compile-time refusal is better than plausible but incorrect Flux"). The curated operation set the
  story suggested (list users, get a user, list groups, list a user's groups, deactivate a user) was
  not authored against endpoints for the same reason C-107's Notes give: authoring paths, schemas and
  risk/idempotency for operations that cannot authenticate would be effort spent on a shape that
  cannot ship, and every one of them would need re-deriving once the auth question is actually
  answered.
- **The eight-red / three-red whole-catalogue pattern does not apply here.** No provider, service or
  operation was added to `providers/`, so `cargo test --workspace --no-fail-fast` shows the
  whole-catalogue staleness checks green, not red — see `## Gate` in the report for the actual run.
  That is itself part of the finding: this story closes with **zero** new red tests, which is the
  signal that nothing was shipped, rather than the eight AGENTS.md tabulates for a story that did.
- **Board not regenerated** — `docs/stories/README.md` is coordinator-owned. `status` moved
  `ready` -> `blocked` here, so the board needs a `/track:board` run at integration.

### 2026-07-31, second pass — the connector ships

Everything above is the first attempt's record and is left intact. C-184 built the prefix axis it
asked for, and commit `3457581` hardened the guard on it; this pass wrote the connector.

- **`providers/okta.toml`, five curated operations**, `base_url = "https://{domain}/api/v1"`:
  `okta-user-list` (the `verify`), `okta-user-get`, `okta-group-list`, `okta-user-group-list`, and
  `okta-user-deactivate` (`risk = "destructive"`, `non_idempotent`). Four reads at `low`/`idempotent`.
  Auth is `scheme = { header = { name = "Authorization", prefix = "SSWS " } }`
  (`providers/okta.toml:96`) — **trailing space included**, and the loader now refuses it without one,
  so the finding the probe recorded as uncatchable is caught.
- **The probe test became the contract test.** `crates/connector-flux/tests/okta_connector.rs` now
  loads the shipped TOML the way `launchdarkly_connector.rs` does. Findings 1 and 3 stay as fixture
  tests, because they are about what the loader *refuses* and a refusal cannot be measured against a
  file that ships. `no_provider_toml_was_shipped_for_this_probe` was inverted rather than deleted, so
  the record reads as one continuous measurement rather than a deletion.
- **Two things this connector cannot express, excluded and asserted rather than commented.**
  `no_curated_operation_offers_a_free_text_filter_or_a_link_header_cursor` fails if a later story adds
  `q`, `filter`, `search` or `after` back. The filters are the C-30 unencodable free-text shape — a
  SCIM expression is *made of* punctuation, quotes and spaces, so it is the catalogue's worst case for
  a query value interpolated verbatim, not a marginal one. `after` is only ever returned in a `Link`
  **response header**, which this model cannot surface, so the parameter would be a knob nobody can
  turn. `limit` ships with `minimum = 1` and **no `maximum`**: Okta's caps differ per endpoint and
  nothing here can check one, so none is invented.
- **`send_email` is the one parameter beyond `limit` and the path ids.** It is `wire = "sendEmail"` on
  the deactivation, and the truthiness gating in emitted Flux happens to be exactly right: `true`
  sends `?sendEmail=true`, anything else sends nothing, which lands on Okta's own default of no email.
  Recorded here because it is a coincidence of the emitter that held, not a property to rely on.
- **No `response_schema` on the deactivation.** Okta answers it with an empty body; declaring a
  permissive placeholder would count toward coverage while telling a consumer nothing, which
  `no_operation_publishes_a_permissive_response_schema` refuses. Same call `providers/miro.toml` makes.
- **Exactly eight red, no ninth.** `the_provider_list_matches_the_repository`,
  `the_catalog_is_not_empty`, `the_committed_tree_is_a_fixed_point_of_a_build`,
  `a_build_plans_both_readme_images_and_they_are_current`, `the_shipped_artifacts_are_byte_identical`,
  `the_published_catalogue_carries_the_service`, `every_shipped_operation_carries_its_metadata_and_its_flux`,
  `the_build_writes_and_checks_site_catalog_json` — the AGENTS.md table exactly.
  `the_recorded_floor_is_the_measured_figure` is **green** here; five operations with four response
  shapes fit inside the slack alone, so this story does not consume the coordinator's ratchet.
- **Fence respected.** `crates/catalog/src/generated.rs`, `web/public/catalog.json`, `COVERED_FLOOR`,
  `CHANGELOG.md`, the board, the roadmap and the lockfiles are untouched. The per-provider
  `crates/catalog/src/generated/okta.rs` and `crates/catalog/ops/okta/*.flux` are committed, as the
  story's `## Notes` says they should be.
- **Board still not regenerated** — `status` moved to `in-progress` here; the coordinator sets `done`
  and runs `/track:board` at integration.
- **Unverified, and named as such:** Okta's per-endpoint `limit` caps, and the exact set of `status`
  and group `type` values. The response schemas describe the documented values in prose and declare
  no `enum`, so a value Okta added later cannot make a consumer reject a valid document.

## Notes

- **Charter fit.** Okta is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/okta.rs` is **not** in that set and is yours to commit.

### Coordinator note at integration

Blocked on [C-184](C-184-auth-scheme-prefix-axis.md), filed from this finding. The probe did its job:
it answered a question for **five** stories at the cost of one, and the answer is now an executable
test rather than a paragraph.

Its split of the five is the part worth keeping: `SSWS` (this story), `Token token=` ([C-162](C-162-provider-pagerduty.md))
and `OAuth` ([C-181](C-181-provider-statuspage.md)) need a prefix axis that was designed in
`docs/designs/unified-auth.md:75-77` and never built. `LaunchDarkly` ([C-175](C-175-provider-launchdarkly.md))
and `ClickUp` ([C-178](C-178-provider-clickup.md)) need **no new capability at all** — a raw
Authorization value is already `AuthScheme::Header { name: "Authorization" }`, which this story's test
proves loads and round-trips. Those two were re-marked `ready` on that evidence and dispatched.
