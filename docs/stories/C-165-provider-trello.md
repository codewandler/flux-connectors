---
id: C-165
title: Ship the Trello connector
pillar: Spec
status: in-progress
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: credential travels in the QUERY STRING (`?key=&token=`). C-159 measured ZERO Placement::Query in the shipped catalogue, and query values are not percent-encoded — the documented gap"
---

# Ship the Trello connector

## Goal

Add Trello to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**The first query-placed credential.** Trello authenticates with `?key=<key>&token=<token>`. C-159 measured the committed catalogue as 18 header placements and 2 inbound — no query placement ships today, and `AGENTS.md` records that query values are not percent-encoded.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** Two query parameters, `key` and `token`, both secret.

**Curated operation set (a starting point, not a mandate):** get a board, list a board's lists, list cards on a list, create a card, archive a card

## Hazards specific to this one

**Read [C-159](C-159-request-debug-and-query-encoding.md) §2 first.** It found that a query-placed credential does not travel as the string registered with the redactor, because `query_encode` escapes `+ / =` — so a base64-ish token can defeat redaction. That makes this connector the one that would make an unreachable bug reachable. Shipping may be the wrong answer; if so, record that, and say what C-159 has to close first.

## Acceptance

- [x] `providers/trello.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
- [x] A `verify` operation that is a read and runs unattended.
- [x] `crates/connector-flux/tests/trello_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses.
- [x] **Failing-first test:** the contract test must fail before `providers/trello.toml` exists.
- [x] The scoped gate is green: `build --provider trello`, `diff --provider trello` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding.

## Notes

- **Charter fit.** Trello is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/trello.rs` is **not** in that set and is yours to commit.

## Progress

**Shipped, not refused — and the refusal was weighed rather than skipped.** 6 operations, 2
query-placed credentials. The full reasoning lives in `providers/trello.toml`'s header comment, where
a reviewer of the connector will actually read it; this is the summary.

**The probe's answer.** The query placement needed *nothing* new: `AuthScheme::Query` was already in
the IR, in the catalogue emitter, in the public-catalogue renderer and in `connector-pack`'s
`place`. The axis was modelled end to end and simply unused — C-159's "18 header, 2 inbound, zero
query" was a fact about the *catalogue*, not about the model. No code changed for this connector.

**On C-159 §2, the reason this story carried a refusal option.** Shipping does make the divergence
reachable: `place` percent-encodes a query credential (`connector-pack/src/auth.rs:162`,
`query_encode` at `:204-215`) while the *unencoded* value is what is registered with the redactor.
It ships anyway, on three grounds — the defect is two lines of `connector-pack` and refusing here
would not fix them; the exposure is exactly one connector and
`trello_connector.rs::trello_is_the_only_query_placement_in_the_shipped_catalogue` fails if a second
ever lands; and nothing here executes any of it, so the exposure begins at the first host that binds
this connector, which is the reader the provider file's header is addressed to. **What C-159 must
close before that host exists:** register the encoded form alongside the raw one (or encode on the
registration side), and item 1 of C-159 — `Request`'s derived `Debug` over `url` — is what makes it
matter here specifically, because for a query credential that `Debug` prints the whole URL.

Deliberately *not* relied on: Trello renders key and token as hex in every published example, and
hex is entirely inside the unreserved set, so `query_encode` would be the identity on it. The vendor
documents no token format, so that is a habit, not a guarantee.

**The second finding, and it is the same wall C-164 hit.** Trello's authorization guide says *"It is
ok for your API key to be publicly available, but a token should never be publicly available"* — and
the only route from `[[config]]` to a query parameter is an `[[auth]]` credential, which forces
`secret = true` (`config.rs:223-231`). So the key is declared **more** protected than the vendor
requires. Accepted, because it is the safe direction and costs this connector nothing: the key has
exactly one destination and is never read back. Algolia could not accept it because its application
id also had to reach a hostname, which needs a non-secret `endpoint.` binding. Filed against
[C-187](C-187-config-cannot-pin-a-request-component.md) as a third instance of one gap — the config
surface reaches a path segment, a query parameter and a header only by way of a secret. The same gap
appears on the *level* axis: Trello's key belongs to a Power-Up registration, but
`Binding::Credential` derives connection level, so the product-wide-key installation is not
expressible and only the personal-registration one is. Both config `help` strings say "from the same
page, in that order" so a key and a token cannot be assembled from two different registrations.

**The curation, and what it is really about.** Not one operation declares a query parameter. This
emitter interpolates query values verbatim (`connector-flux/src/op.rs:138-143`), and on a connector
whose credential lives in the query string that gap is sharper than the standing
`zendesk-ticket-search` case: a caller value carrying `&` would be interpolated *ahead* of the
credential the host appends. Trello's own reference supplies the way out — its card parameters "may
also be replaced with a JSON request body instead" — so both writes carry their free text in a JSON
body, and the query string of every emitted request is empty until the host appends `key` and
`token`. Left out for that reason and named rather than silently absent: `fields`, `filter`, the
nested `cards`/`lists` expansions, and `before`/`since`.

**Endpoints verified against developer.atlassian.com on 2026-07-31**, not recalled:
`GET /1/members/me/boards` (the API introduction's own worked `curl` example, which is why it, and
not `GET /1/members/me`, is the argument-free `verify` — the members reference does not document the
literal `me`), `GET /1/boards/{id}`, `GET /1/boards/{id}/lists`, `GET /1/lists/{id}/cards`,
`POST /1/cards`, `PUT /1/cards/{id}`. Response shapes are the vendor's own documented Board and Card
examples. `trello-board-lists` carries **no** `response_schema`: Trello documents the endpoint but
publishes no List example, and this repository declares the shape the vendor documents or declares
nothing.

**Unverified, and therefore left out:** `GET /1/lists/{id}` (no documented response body, and it
adds nothing `trello-board-lists` returns), `POST /1/lists`,
`POST /1/lists/{id}/archiveAllCards`, `DELETE /1/cards/{id}`, and the whole of comments, checklists,
labels, attachments and webhooks. No `[[events]]`/`[[channels]]`: Trello does publish a webhook API,
and an inbound half must also state how it is registered and verified, which is its own story.

**Eight red, five binaries, no ninth** — measured with `cargo test --workspace --no-fail-fast`, and
exactly the set `AGENTS.md` tabulates: `the_provider_list_matches_the_repository` and
`the_catalog_is_not_empty` (`catalog::embedded_operations`),
`the_committed_tree_is_a_fixed_point_of_a_build` (`connector-cli::catalog_artifacts`),
`a_build_plans_both_readme_images_and_they_are_current` (`connector-cli::readme_snippet`),
`the_shipped_artifacts_are_byte_identical` and `the_published_catalogue_carries_the_service`
(`connector-cli::service_units`), `every_shipped_operation_carries_its_metadata_and_its_flux` and
`the_build_writes_and_checks_site_catalog_json` (`connector-cli::site_catalog`).
`the_recorded_floor_is_the_measured_figure` is **green** here: this connector adds 5 covered
operations of 6, well inside the floor's slack. `COVERED_FLOOR` was not touched.
