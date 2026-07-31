---
id: C-173
title: Ship the Typeform connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "responses are cursor-paginated with a `before`/`after` token pair, and the response payload is answer-shaped rather than record-shaped"
---

# Ship the Typeform connector

## Goal

Add Typeform to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**Cursor pagination as a declared parameter pair.** Typeform's responses endpoint pages with `before`/`after`. The member contract already requires a poll binding to have a cursor; this is the operation-level version.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <token>`.

**Curated operation set (a starting point, not a mandate):** list forms, get a form, list responses, delete responses (destructive), get form insights

## Hazards specific to this one

Do not model an inbound webhook surface here unless you have read [C-158](C-158-typescript-catalogue-types-drift.md)'s last note: the manifest round-trip tests read the **default-service** manifest, so a multi-service provider with an inbound surface panics in two of them. Single-service is the safe shape for this story.

## Acceptance

- [x] `providers/typeform.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. Five operations: `typeform-user-me`,
      `typeform-form-list`, `typeform-form-get`, `typeform-response-list`, `typeform-response-delete`.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      One field, `access_token`.
- [x] A `verify` operation that is a read and runs unattended. `typeform-user-me`, `GET /me`, no params.
- [x] `crates/connector-flux/tests/typeform_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses.
      `the_cursor_pair_survives_because_it_avoids_the_pipelines_danger_set` is the archetype assertion.
- [x] **Failing-first test:** the contract test must fail before `providers/typeform.toml` exists. See
      `BASE_PROOF` in the implementation report.
- [x] The scoped gate is green: `build --provider typeform`, `diff --provider typeform` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. Confirmed: exactly the
      eight AGENTS.md names, no more, no fewer. `the_recorded_floor_is_the_measured_figure` (the
      documented ninth) stayed green — coverage landed at 137/156 (87%), inside the ratchet's slack.

## Progress

- Shipped 5 operations, not the story's suggested 5 (list forms, get a form, list responses, delete
  responses, get form insights) — `get form insights` (`GET /insights/{form_id}/summary`) is excluded.
  The path is corroborated by multiple independent sources, but its response body shape could not be
  corroborated with enough confidence from reachable documentation to declare a `response_schema`
  honestly, and it is additionally gated behind Typeform's Business plan. Recorded as a deliberate
  exclusion in `providers/typeform.toml`'s header comment and in
  `the_curated_operation_set_is_the_one_the_story_selected`'s assertion message, per this story's own
  "a confident four beats a guessed ten" instruction. `typeform-user-me` (`GET /me`, the account the
  token belongs to) fills the fifth slot instead, as the connector's `verify` operation.
- **The central finding:** Typeform's response `token` (what `before`/`after` page against) is a
  fixed 32-character lowercase-hex string. This is corroborated by an observed example in Typeform's
  own JSON-response-explanation documentation and a named ex-Typeform engineer's statement on the
  vendor's own community forum, but **not** by a published, versioned spec — this repository vendors
  no Typeform OpenAPI/JSON description under `specs/`. `providers/typeform.toml`'s header comment
  states this confidence gap explicitly and names the fail-closed consequence if the format ever
  widens (a schema-rejected call, not a corrupted query string).
- Unverified / left out, named rather than guessed: `workspace_id` on `typeform-form-list` (filter
  exists; character set not confidently known); `logic` and `variables` on `typeform-form-get`'s
  response (real fields; nested shape not confidently known); `fields`/`included_response_ids`/
  `excluded_response_ids` as *list*-operation filters (character-set-safe in principle by the same
  hex-token reasoning, but left out to keep the curated list operation to the archetype plus the safe
  basics — `included_response_ids` is declared once, where the connector cannot function without it,
  on the delete operation); the exact response body of `typeform-response-delete` (Typeform's own
  reference does not detail one beyond confirming `200 OK` registers the request — no `response_schema`
  declared, per the `zoom-meeting-delete` convention).
- No inbound webhook surface modelled, per the story's own hazard note (C-158) — no `[[services]]`,
  `[[events]]` or `[[channels]]` anywhere in this file, asserted by `no_inbound_surface_is_declared`.
- No PII: no operation, description, or test fixture in this diff carries an example answer value,
  email address or name. `no_response_field_carries_an_example_value` holds this as a property of the
  whole file, not a promise kept by inspection alone.

## Notes

- **Charter fit.** Typeform is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/typeform.rs` is **not** in that set and is yours to commit.
