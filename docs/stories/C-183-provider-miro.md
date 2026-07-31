---
id: C-183
title: Ship the Miro connector
pillar: Spec
status: in-progress
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "board items are a discriminated union by `type` (sticky_note, shape, text, frame) — the same shape problem as Notion's blocks, one level shallower and possibly expressible"
---

# Ship the Miro connector

## Goal

Add Miro to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A shallow discriminated union.** Miro's items are typed variants, but unlike Notion's blocks they are **not recursive**. This is the test of whether the union was refused for being a union or for being recursive.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Bearer <access_token>`.

**Curated operation set (a starting point, not a mandate):** list board items, get an item, create a sticky note, update an item, delete an item (destructive)

## Hazards specific to this one

Read C-107's block-model refusal in `providers/notion.toml` first and state explicitly which of its two reasons apply here. If a non-recursive union is expressible, that narrows Notion's recorded gap and is worth noting in this story's Progress — it is evidence about the model, not just a connector.

## Acceptance

- [x] `providers/miro.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. → `providers/miro.toml`, 6
      operations: `miro-board-list`, `miro-board-item-list`, `miro-board-item-get`,
      `miro-sticky-note-create`, `miro-sticky-note-update`, `miro-sticky-note-delete`.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. → every `[[operations]]` block in `providers/miro.toml`;
      effects are the emitter's own `network` tag (`crates/connector-flux/src/op.rs:616`), not authored.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      → the single `access_token` field, asserted by
      `the_access_token_is_configurable_and_carries_no_example_value`.
- [x] A `verify` operation that is a read and runs unattended. → `verify = "miro-board-list"`, asserted
      by `the_connector_verifies_with_a_read_over_a_bearer_token`.
- [x] `crates/connector-flux/tests/miro_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. → 12 tests,
      including `the_read_side_union_is_expressed_as_a_oneof_over_the_four_item_types` and
      `the_write_side_never_declares_a_type_discriminator_body_field`, the direct assertions of the
      archetype question.
- [x] **Failing-first test:** the contract test must fail before `providers/miro.toml` exists. → see
      `BASE_PROOF` in the handoff report; all 12 tests failed on "cannot read providers/miro.toml"
      before the file existed.
- [x] The scoped gate is green: `build --provider miro`, `diff --provider miro` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. → all green;
      see the handoff report's `GATE`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. → exactly the eight
      `AGENTS.md` tabulates, measured with `--no-fail-fast`; see `## Progress`. A ninth,
      `the_recorded_floor_is_the_measured_figure`, stayed **green** in this worktree — this story alone
      fits inside C-166/C-171's already-spent slack, so it is not a tenth red test, only a note for the
      coordinator's accumulation across the wave.

## Notes

- **Charter fit.** Miro is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/miro.rs` is **not** in that set and is yours to commit.

## Progress

**Which of Notion's two refusal reasons apply here — the question this story was chosen to
answer.** `providers/notion.toml`'s header comment (around line 60) refuses the block model for two
independent reasons:

1. *"`JsonSchema` here has no `$ref` and no recursion, so that union is not expressible."* **Does not
   apply.** Miro's items are not recursive: a frame's members reference it back through their own
   `parent.id`; a frame never embeds an array of nested item bodies the way a Notion `column_list`
   embeds `column`s that embed blocks. No `$ref` is ever needed to describe it.
2. *"`body_schema` could take it as a free-form object, but that ships an untyped blob... precisely
   the 'plausible but incorrect' output AGENTS.md says to refuse."* This is the one that could have
   applied — if a write had to carry the union in a request body. It doesn't, here: Miro's own API
   resolves the discriminator through the **URL** (`POST /boards/{id}/sticky_notes`, not a generic
   `POST /boards/{id}/items` with a `type` field in the body), so `miro-sticky-note-create`/`-update`
   are plain flat bodies, never a union. This reason simply never gets a chance to bite on the write
   side.

**The finding, stated plainly: a shallow, non-recursive union *is* expressible in this pipeline — just
not through the mechanism that blocked Notion.** On the read side (`miro-board-item-list`,
`miro-board-item-get`), Miro really does answer with a heterogeneous, caller-uncontrolled `type`-tagged
value, and this connector expresses that union directly as a JSON Schema `oneOf` over all four
variants in `response_schema`. That works *because* `response_schema` is a raw `serde_json::Value`
(`crates/connector-spec/src/ir.rs:675`) with none of `params.body`'s `BodyNode`/flat-parameter-list
constraints — `oneOf` is even explicitly listed as one of the keys that makes a schema *informative* by
`crates/connector-spec/tests/response_schema_coverage.rs`'s own `is_permissive`. Notion's blocks were
never blocked by "union-ness" as such; recursion specifically is what `$ref`-free `JsonSchema` cannot
do, and a bounded, non-recursive `oneOf` carries no such requirement. So this narrows Notion's recorded
gap rather than just avoiding it: the write-side union dissolves because the vendor's own routing
resolves it before this pipeline's schema ever has to, and the read-side union was expressible all
along, just never exercised by an existing connector. `crates/connector-flux/tests/miro_connector.rs`
asserts both halves directly (`the_read_side_union_is_expressed_as_a_oneof_over_the_four_item_types`,
`the_write_side_never_declares_a_type_discriminator_body_field`) rather than leaving this as prose.

**C-186 (idempotent POST/PATCH) is hit for real, on the PATCH side.**
`miro-sticky-note-update` is genuinely idempotent by Miro's documented behaviour — it always sets an
absolute `content` value, never a delta — but `check_write_metadata` refuses `idempotency =
"idempotent"` on any `PATCH` by method alone, so it is declared `non_idempotent` and the true
behaviour is recorded in the provider file's header and operation comment instead, exactly the trade
C-186 exists to fix. `the_sticky_note_update_is_forced_non_idempotent_by_the_patch_rule` proves the
emitter's refusal directly (attempting to declare it `idempotent` fails to emit).

**C-185 (array bodies) was never needed.** Every request body in this connector is a single,
flat-nested object (`data.content`) — a single sticky note has no array-shaped field to send — so this
connector does not exercise the `BodyNode`-array gap C-185 tracks. Noted per the dispatch, not because
it came up.

**Unverified / not independently confirmed against a live account or the vendor's OpenAPI document**
(no network access in this build; written from documented Miro API v2 endpoint shapes and excluded
rather than guessed further where uncertain):

- The exact response envelope fields beyond `id`/`type`/`data`/`position`/`geometry` (e.g. `org`,
  `createdBy`, `modifiedBy`, `links`) are omitted from `response_schema` rather than guessed at.
- `style` (fill colour, text alignment) on sticky-note create/update is excluded outright — Miro
  documents a named colour palette but this file is not confident of the complete, current
  enumeration, and declaring a wrong `enum` would reject a real value.
- Pagination (`cursor`/`limit`) on `miro-board-list` and `miro-board-item-list` is excluded; this file
  returns Miro's first page only, the same choice `providers/cloudflare.toml` makes for its own list
  endpoints and for the same reason (not confident of the exact bounds).
- Whether Miro answers a repeat `DELETE` on an already-deleted sticky note with `404` or a repeated
  `204` was not independently confirmed; `miro-sticky-note-delete` is declared `non_idempotent`
  following `providers/cloudflare.toml`'s identical precedent for its DNS record delete, on the
  reasoning that an unconfirmed idempotency claim is not one this file should make.
- Shapes, text items and frames as **write** surfaces (create/update/delete) are out of scope for this
  curated set — a confident follow-on, not a gap. The read side already covers all four types because
  it is generic across them.
