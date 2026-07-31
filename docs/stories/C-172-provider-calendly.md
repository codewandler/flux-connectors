---
id: C-172
title: Ship the Calendly connector
pillar: Spec
status: in-progress
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "every resource is addressed by a full URI, not an id — so a parameter's value is itself a URL, and the template must not double-compose it"
---

# Ship the Calendly connector

## Goal

Add Calendly to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**URIs as parameters.** Calendly identifies a user or event type by its full `https://api.calendly.com/…` URI, which is then passed as a *query value*. A template that composes a URL from a value that is already a URL is the hazard.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <token>`.

**Curated operation set (a starting point, not a mandate):** get the current user, list event types, list scheduled events, get a scheduled event, list invitees

## Hazards specific to this one

A URI-valued query parameter is exactly the case the unencoded-query gap bites: it contains `:` and `/` and possibly `?`. Establish whether it survives, and if it does not, that is the finding. `get the current user` (`/users/me`) is the natural `verify`. **Invitee data is personal data — author no example values.**

## Acceptance

- [x] `providers/calendly.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. Five operations: `calendly-user-me`,
      `calendly-event-type-list`, `calendly-scheduled-event-list`, `calendly-scheduled-event-get`,
      `calendly-invitee-list`.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. All five are reads: `risk = "low"`,
      `idempotency = "idempotent"`.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`
      (`providers/calendly.toml`'s single `access_token` field, `secret = true` /
      `binds = "credential.calendly.access_token"`).
- [x] A `verify` operation that is a read and runs unattended (`calendly-user-me`, `GET /users/me`, no
      argument).
- [x] `crates/connector-flux/tests/calendly_connector.rs` — a per-provider contract test asserting the
      URI-survives-verbatim claim and the path-parameter double-composition guard, not that the file
      parses.
- [x] **Failing-first test:** confirmed — all 8 tests in the file fail with "cannot read
      .../providers/calendly.toml" before the TOML existed (see report `BASE_PROOF`).
- [x] The scoped gate is green: `build --provider calendly`, `diff --provider calendly` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — measured: `the_provider_list_matches_the_repository`,
      `the_catalog_is_not_empty`, `the_committed_tree_is_a_fixed_point_of_a_build`,
      `a_build_plans_both_readme_images_and_they_are_current`, `the_shipped_artifacts_are_byte_identical`,
      `the_published_catalogue_carries_the_service`, `every_shipped_operation_carries_its_metadata_and_its_flux`,
      `the_build_writes_and_checks_site_catalog_json` — the exact eight AGENTS.md tabulates, no more, no
      fewer.

## Progress

**The finding: a URI-valued query parameter survives this pipeline's unencoded-query gap intact, so
Calendly ships as a real connector rather than a recorded refusal.**

`crates/connector-flux/src/op.rs:138-141` names the exact danger set: "a value carrying a space, `&`,
`#` or `=` corrupts the query string." `docs/designs/query-encoding-flux-stories.md`'s own measured
table adds the empirical detail — a colon reaches the wire unmangled today ("`type:ticket status:new`"
→ "correct, by accident"). A Calendly resource URI (`https://api.calendly.com/{resource}/{uuid}`) is
built only from a scheme, a fixed host, path segments and hyphens — it structurally cannot contain any
of the four dangerous characters, unlike the free-text Zendesk search term that motivated the gap in
the first place. `crates/connector-flux/tests/calendly_connector.rs::a_uri_valued_query_parameter_survives_because_it_avoids_the_pipelines_danger_set`
makes this mechanical rather than asserted: it checks the declared `pattern` on every `user` query
parameter against the same four-character set, and confirms the emitted Flux (`cargo run -p
connector-cli -- build --provider calendly`) interpolates it verbatim — `url =
fmt("{base}/event_types?user={user}")`, observed directly in
`crates/catalog/ops/calendly/calendly-event-type-list.flux`.

The inverse hazard the story also names — a path parameter accepting the whole URI would
double-compose the path template — is closed by constraining `calendly-scheduled-event-get`'s and
`calendly-invitee-list`'s `uuid` path parameter to a 36-character hex-and-hyphen pattern that admits no
scheme, so the full URI is rejected by the parameter's own schema rather than silently producing a
malformed request.

**Verified against the live API, not just against memory:** `GET https://api.calendly.com/users/me`
with no credential returns `401 Unauthorized` (a real response from the real host, confirming the path
exists), and web search corroborated the `/event_types?user={uri}`, `/scheduled_events`, and
`/scheduled_events/{uuid}/invitees` shapes, including a concrete confirmation that an event's
`invitees` collection is reached by appending `/invitees` to the event's own URI
(`https://api.calendly.com/scheduled_events/{uuid}/invitees`) — matching this connector's path template
exactly. Response envelope shapes (`{"resource": {...}}` for a single item, `{"collection": [...],
"pagination": {...}}` for a list) and the error shape (`{"title": ..., "message": ...}`) are recorded
from documentation and community examples rather than a live authenticated call, since authoring this
connector required no credential and none was available.

**Deliberately excluded, named rather than silently dropped** (see `providers/calendly.toml`'s header
comment for the full reasoning):
- Cursor pagination (`page_token`) — an opaque server-issued token this pipeline cannot safely
  re-encode, the same reasoning that excludes Asana's `offset` and Notion's `start_cursor`.
- `min_start_time` / `max_start_time` — an ISO 8601 offset (`+05:00`) carries the *other*
  unencoded-query hazard (`+` read as a space by a `application/x-www-form-urlencoded` decoder), and
  folding a second hazard into this connector would blur the one finding it was chosen to demonstrate.
- Filtering invitees by `email` — the same `+`-in-email hazard, and it would ask a caller to name a
  specific third party as an argument, which the personal-data instruction rules out.
- Any write operation (creating/canceling an event, inviting a guest) — out of scope for the curated
  read-only set this story selected.

**Unverified, named honestly:** the exact JSON field lists for `location` (Calendly documents several
location kinds — video call, phone, in person, custom — with different shapes) and the precise set of
optional fields on `/users/me` and `/event_types` beyond what is declared. Left loosely typed or
omitted rather than guessed, following `zendesk.toml`'s and `asana.toml`'s precedent for a field the
vendor's own reference leaves ambiguous.

## Notes

- **Charter fit.** Calendly is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/calendly.rs` is **not** in that set and is yours to commit.
