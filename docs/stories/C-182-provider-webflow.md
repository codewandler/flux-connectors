---
id: C-182
title: Ship the Webflow connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "a CMS item's fields are a free-form object whose shape is defined by a user's collection schema — genuinely unknowable at compile time, which is the honest limit of a typed connector"
---

# Ship the Webflow connector

## Goal

Add Webflow to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A payload this pipeline cannot type.** A Webflow CMS item's `fieldData` is whatever the site owner's collection defines. Notion's block model was excluded for being recursive; this is excluded-or-untyped for being *user-defined*.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <token>`.

**Curated operation set (a starting point, not a mandate):** list collections, get a collection schema, list items, get an item, publish a site

## Hazards specific to this one

The interesting decision is whether item *creation* ships at all. C-107 excluded Notion's blocks rather than shipping an untyped blob, and the same reasoning applies: `get a collection schema` lets a caller discover the shape at runtime, which may be the honest substitute for typing it. Say which you chose and why.

## Acceptance

- [x] `providers/webflow.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/webflow.toml`, 6
      operations (site list/verify, collection list, collection get, item list, item get, site publish).
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every `[[operations]]` block in
      `providers/webflow.toml`; effects are the standard `network` tag the emitter adds.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → `providers/webflow.toml`'s single `[[config]]` block (`token`), asserted by
      `the_token_is_configurable_and_carries_no_example_value`.
- [x] A `verify` operation that is a read and runs unattended. → `verify = "webflow-site-list"`, a
      parameter-free `GET`, asserted by `the_connector_verifies_with_a_read_over_a_bearer_token`.
- [x] `crates/connector-flux/tests/webflow_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. → 13 tests,
      centered on `item_creation_is_not_shipped_because_fielddata_is_tenant_defined`,
      `collection_get_ships_as_the_runtime_substitute_for_typing_fielddata` and
      `fielddata_is_declared_open_in_every_item_read`.
- [x] **Failing-first test:** the contract test must fail before `providers/webflow.toml` exists. →
      proved in-worktree by moving the file aside; see `BASE_PROOF` in the handoff report.
- [x] The scoped gate is green: `build --provider webflow`, `diff --provider webflow` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. → all green,
      see the handoff report's `GATE` section.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly eight, matching
      AGENTS.md's table verbatim; no ninth (`response_schema_coverage` stayed green — see Progress).

## Progress

**Decision: item creation does NOT ship.** `providers/webflow.toml`'s header comment gives the full
reasoning; summary: a collection item's `fieldData` is a flat, tenant-defined object — neither
recursive (so Notion's block-model refusal doesn't strictly apply) nor a small bounded union (so
Miro's `oneOf`-on-the-read-side trick doesn't apply either, because there is no enumerable set of
shapes to write down at all — it is unbounded and per-collection). `webflow-collection-get` ("get a
collection schema") ships instead as the honest runtime-discovery substitute: it returns the
collection's own `fields` array (id, slug, displayName, type, isRequired), which is exactly what a
caller needs to learn `fieldData`'s shape before reading or (elsewhere) writing one. Full read access
to items ships (list + get) with `fieldData` declared as an open, undescribed `object` inside an
otherwise fully-typed envelope — coverage-safe under `response_schema_coverage.rs`'s `is_permissive`
check, which inspects only the operation's top-level schema.

A second, independent reason also excludes item creation: Webflow's create endpoint
(`POST /v2/collections/{collection_id}/items`) is envelope/array-shaped (creating even one item wraps
it), which `BodyNode` (`crates/connector-flux/src/op.rs`) cannot express — it builds nested objects via
a dotted `wire` path and never an array, at any depth (C-185, which names this exact connector as a
predicted hit). Either reason alone would have excluded the operation.

**Unverified / not vendored, named rather than guessed:**
- The exact JSON shape of `POST /sites/{site_id}/publish`'s response body. No `response_schema` is
  declared for `webflow-site-publish` for this reason (absence over a guessed placeholder).
- `POST /sites/{site_id}/publish`'s optional `customDomains` selector's precise shape (believed to be
  an array of domain ids) — not asserted; the operation sends no body at all and always publishes to
  every connected domain, which is documented in the operation's `description`.
- The vendor's error envelope shape. No `quirks.error_envelope` is declared (matching
  `providers/miro.toml`'s precedent of omitting it rather than guessing the pointer).
- Exact default page size / upper bound for `offset`/`limit` pagination on the two list reads — both
  are excluded entirely rather than guessed at (matching `providers/cloudflare.toml` and
  `providers/miro.toml`'s stance).

Item delete (`DELETE /v2/collections/{collection_id}/items/{item_id}`) would not need `fieldData` and
could in principle ship on path parameters alone, but was left out of this first curated set — the
story's given set is reads plus the one necessary write (publish), not a mandate to add every
independently-safe write.

`response_schema_coverage_does_not_fall_below_its_floor` and `the_recorded_floor_is_the_measured_figure`
(the ratchet and its coordinator-owned upward twin) both stayed green: 5 of this connector's 6
operations carry a `response_schema` (only `webflow-site-publish` does not, for the reason above), and
that addition did not push coverage past the floor's 10%-of-catalogue slack. Neither test needed
touching, and `COVERED_FLOOR` was not touched.

## Notes

- **Charter fit.** Webflow is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/webflow.rs` is **not** in that set and is yours to commit.
