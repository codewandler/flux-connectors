---
id: C-164
title: Ship the Algolia connector
pillar: Spec
status: blocked
design:
epic: provider-fleet-2
areas: [providers]
note: "blocked — measured, not predicted. `ConfigField::binds` parses to exactly one of five destinations (crates/connector-spec/src/config.rs:178-202,239-267) and none is a request header; the one route that reaches a header — an `[[auth]]` credential — forces `secret = true` on whatever config field binds it (provider.rs:609-629), and the application id is not a secret. The hostname and the header cannot share one declared value; filed as a finding for C-187."
---

# Ship the Algolia connector

## Goal

Add Algolia to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A configured host plus a second credential.** Algolia sends `X-Algolia-API-Key` and `X-Algolia-Application-Id`, and the application id *also* forms the hostname. One declared value has to reach two places.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** Two headers: `X-Algolia-API-Key` (secret) and `X-Algolia-Application-Id` (not secret).

**Curated operation set (a starting point, not a mandate):** search an index, get an object, list indices, save an object, delete an object (destructive)

## Hazards specific to this one

The application id is **not** a secret, so `secret` must disagree with the API key's — the configuration contract requires `secret` to agree with `binds`, so get that pairing right. Depends on the same configured-host question as [C-163](C-163-provider-salesforce.md); coordinate rather than both discovering it.

## Acceptance

- [ ] `providers/algolia.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. **Not done, deliberately** — see
      `## Progress`. Every curated operation needs `X-Algolia-Application-Id` on the wire, and no
      declared value can reach both that header and the `{app_id}-dsn.algolia.net` hostname honestly
      with today's config surface, so shipping the TOML would either ask an operator for the same
      value twice with no guard against a mismatch, or mislabel a public identifier as a secret.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. **N/A** — no operations authored, for the same reason.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      **N/A** — same reason.
- [ ] A `verify` operation that is a read and runs unattended. **N/A** — same reason.
- [x] `crates/connector-flux/tests/algolia_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. **Done, in
      the shape the answer actually took**: it asserts the two-position question directly (the closed
      `Binding` enum, the credential route's forced `secret = true`, and the caller-supplied header
      parameter's disconnection from `[[config]]`) rather than loading a `providers/algolia.toml` that
      does not exist.
- [ ] **Failing-first test:** the contract test must fail before `providers/algolia.toml` exists. **Not
      applicable in its literal form** — there is no `providers/algolia.toml` to gate on. The nearest
      honest equivalent: at `$(git merge-base main HEAD)` the test file itself does not exist (`cargo
      test -p connector-flux --test algolia_connector` errors `no test target named
      algolia_connector`); see `BASE_PROOF` in the report.
- [x] The scoped gate is green: `build --provider algolia`, `diff --provider algolia` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. See `GATE` in
      the report — `build`/`diff --provider algolia` correctly refuse (no such provider), everything
      else is green including the new test.
- [ ] **Exactly eight tests are red and reported, not silenced.** **Does not apply, and that is the
      finding**: no provider, service or operation was added to `providers/`, so the whole-catalogue
      staleness checks stay green. Zero new red tests, not eight, is the correct count for a story that
      shipped no provider.

## Progress

- **2026-07-31 — attempted, blocked at the config surface. Nothing shipped, deliberately.** The probe
  question this story exists to answer — *can one declared value reach both the hostname and a
  header?* — is **no**, and it was measured against the loader rather than read off the design doc.

  Two of the three prerequisites were already answered before this story ran, so this Progress note
  does not re-derive them: two credentials on one request is expressible (`AuthRequirement::all`, C-160
  / Datadog) and a configured host is expressible (`Binding::Endpoint`, C-163 / Salesforce). What
  remained was whether Algolia's application id — required in *both* the hostname
  (`{app_id}-dsn.algolia.net`) and the `X-Algolia-Application-Id` header, and **not secret** — could be
  declared once and reach both.

  1. **`ConfigField::binds` parses to exactly one of five destinations, and a request header is not
     one of them.** `crates/connector-spec/src/config.rs:178-202`:

     ```rust
     pub enum Binding<'a> {
         Endpoint { variable: &'a str },
         Credential { name: &'a str },
         Username { name: &'a str },
         OAuthClientId,
         OAuthClientSecret,
     }
     ```

     `parse_binding` (`config.rs:239-267`) accepts only `endpoint.`, `credential.`, `username.`,
     `oauth.client_id` and `oauth.client_secret`, and refuses everything else —
     `crates/connector-flux/tests/algolia_connector.rs::config_binding_has_no_header_destination`
     measures it directly against a `header.`-shaped string that was never given a spelling.
  2. **The one route that *can* place a value in an arbitrary request header — a declared `[[auth]]`
     credential — forces `secret = true` on whatever `[[config]]` field binds it, unconditionally.**
     `Binding::is_secret` (`config.rs:223-231`) returns `true` for `Credential` with no exception, and
     the loader enforces the agreement rather than trusting it
     (`crates/connector-spec/src/provider.rs:609-629`). A `[[config]]` field binding
     `credential.algolia.application_id` while declaring `secret = false` — the true fact, since
     Algolia documents the application id as safe to embed in client-side code alongside a
     search-only key — is refused for exactly that contradiction. `algolia_connector.rs`'s
     `routing_the_application_id_through_a_credential_forces_a_false_secret_claim` proves the refusal
     fires rather than assuming it.
  3. **The endpoint binding reaches the hostname and nothing else.** Binding `endpoint.app_id` loads
     cleanly and resolves `{app_id}` in `base_url` — this is exactly the shape C-163 shipped. But
     `ParamSet::header` (`crates/connector-spec/src/ir.rs:259-266`) is a **caller-supplied** parameter,
     filled in by a model on every call, with no link back to `[[config]]` at all. Declaring the same
     header there does not pin it to the config value; it only gives the operator (or a model acting
     for one) a second, unconnected place to type the same string.
     `the_endpoint_binding_reaches_only_the_host_and_a_header_parameter_is_a_separate_per_call_value`
     measures this: the binding resolves correctly, and no operation in the fixture has any way to
     reach it from a header.

  So the two positions cannot share one declared value today, and the two ways to *not* share it are
  both bad: asking the operator to paste the application id twice (once as `endpoint.app_id`, once as
  a mislabelled `credential.algolia.application_id`) risks a silent mismatch that produces a confusing
  vendor error neither declaration explains: or omitting the header entirely and shipping a connector
  that fails closed on every real call, which the story's own framing ranks below a recorded refusal
  ("that attempt was worth more than a connector that answered 400"). Recording the refusal was the
  chosen path.

- **Filed as a finding for [C-187](C-187-config-cannot-pin-a-request-component.md).** That story
  already tracks two motivating cases where `ConfigField::binds` cannot reach a request component —
  Cloudflare's `zone_id` (a path segment, C-169) and Vercel's `teamId` (a query parameter, C-170) — and
  its own Notes already flag the header case as worth checking: *"Worth checking while here: whether a
  **header** can be operator-pinned... nothing pins a header the operator knows."* This story is the
  answer to that open question, met by a real connector rather than a hypothetical. C-187 is a shared
  ledger this story's fence does not permit editing directly, so the finding is recorded here for the
  coordinator to fold in at integration: a non-secret, operator-known value has no route into a
  request header today, the same gap C-187 already names for a path segment and a query parameter.
- **No `providers/algolia.toml` shipped, and no crate other than the new test file touched.** Shipping
  a connector whose only way to send the required header would be to either duplicate the value under
  a false `secret = true` claim, or omit the header and fail closed on every call, is the exact failure
  mode AGENTS.md's non-negotiable rules exist to avoid ("a loud compile-time refusal is better than
  plausible but incorrect Flux"). The curated operation set the story suggested (search an index, get
  an object, list indices, save an object, delete an object) was not authored against endpoints for the
  same reason C-107's and C-161's Progress notes give: authoring paths, schemas and risk/idempotency
  for operations that cannot authenticate honestly would be effort spent on a shape that cannot ship,
  and every one of them would need re-deriving once the config question is actually answered.
- **The eight-red / three-red whole-catalogue pattern does not apply here.** No provider, service or
  operation was added to `providers/`, so `cargo test --workspace --no-fail-fast` shows the
  whole-catalogue staleness checks green, not red — see `GATE` in the report. That is itself part of
  the finding: this story closes with **zero** new red tests, which is the signal that nothing was
  shipped, rather than the eight AGENTS.md tabulates for a story that did.
- **Board not regenerated** — `docs/stories/README.md` is coordinator-owned. `status` moved `ready` ->
  `blocked` here, so the board needs a `/track:board` run at integration.

## Notes

- **Charter fit.** Algolia is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/algolia.rs` is **not** in that set and is yours to commit.
