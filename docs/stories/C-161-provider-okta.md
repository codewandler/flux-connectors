---
id: C-161
title: Ship the Okta connector
pillar: Spec
status: blocked
priority: 2
design:
epic: provider-fleet-2
areas: [providers]
note: "blocked — measured, not predicted. `AuthScheme` is a closed five-variant enum (crates/connector-spec/src/auth.rs:70-102) with no prefix axis; Okta's `Authorization: SSWS <token>` cannot be expressed honestly without either extending that enum (a connector-spec change four other auth stories are also waiting on) or baking `SSWS ` into a credential value, which AGENTS.md forbids outright"
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

- [ ] `providers/okta.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. **Not done, deliberately** — see
      `## Progress`. Every curated operation needs `Authorization: SSWS <token>`, and that scheme is
      not expressible honestly with today's `AuthScheme`, so shipping the TOML would ship a connector
      that fails closed with `401` on every call.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. **N/A** — no operations authored, for the same reason.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      **N/A** — same reason.
- [ ] A `verify` operation that is a read and runs unattended. **N/A** — same reason.
- [x] `crates/connector-flux/tests/okta_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. **Done, in
      the shape the answer actually took**: it asserts the auth-scheme question directly (the closed
      `AuthScheme` enum, the header scheme's missing prefix axis, and the credential-value trap) rather
      than loading a `providers/okta.toml` that does not exist.
- [ ] **Failing-first test:** the contract test must fail before `providers/okta.toml` exists. **Not
      applicable in its literal form** — there is no `providers/okta.toml` to gate on. The nearest
      honest equivalent: at `$(git merge-base main HEAD)` the test file itself does not exist (`cargo
      test -p connector-flux --test okta_connector` errors `no test target named okta_connector`); see
      `BASE_PROOF` in the report.
- [x] The scoped gate is green: `build --provider okta`, `diff --provider okta` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. See `GATE` in
      the report — `build`/`diff --provider okta` correctly refuse (no such provider), everything else
      is green including the new test.
- [ ] **Exactly eight tests are red and reported, not silenced.** **Does not apply, and that is the
      finding**: no provider, service or operation was added to `providers/`, so the whole-catalogue
      staleness checks stay green. Zero new red tests, not eight, is the correct count for a story that
      shipped no provider.

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
     `the_header_scheme_carries_no_prefix_to_smuggle_ssws_onto` in the same file.
  3. **`docs/designs/unified-auth.md:75-77` proposed exactly this field** — `prefix` on header
     placement, called "the single highest-value element of this whole design" because it turns
     `Bearer `, `Basic `, `Token ` and `GenieKey ` into one code path — **and it was never
     implemented.** The shipped `AuthScheme::Header` mirrors flux's own four-variant vocabulary
     (`auth.rs:48-58`) exactly, on purpose, so the seam that would carry `SSWS ` does not exist on
     either side today.
  4. **A bare `header` placement aimed at `Authorization` loads — and that is the trap, not the
     answer.** `AuthScheme::Header` does not know or care what header name it is given, so
     `scheme = { header = { name = "Authorization" } }` is legal
     (`the_header_scheme_would_load_but_cannot_honestly_spell_okta_s_prefix`). But the header's whole
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
    }` is exactly what `the_header_scheme_would_load_but_cannot_honestly_spell_okta_s_prefix` proves
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
