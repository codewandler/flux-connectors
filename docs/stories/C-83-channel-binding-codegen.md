---
id: C-83
title: Publish events and channel bindings into the manifest and the catalogue
pillar: Codegen
status: done
design: docs/designs/channel-bindings.md
epic: channel-bindings
areas: [connector-cli, catalog, web]
note: "the strict split: bindings reach the manifest and catalog.json and NOTHING reaches the module. The emitter must refuse to dress a binding up as a pollable op"
---

# Publish events and channel bindings into the manifest and the catalogue

## Goal
Make the third member kind visible to consumers. Events and bindings are in the IR and in the hash
domain; they currently reach no artifact, so a host has no way to read what a connector declares.

## Acceptance
- [x] `connectors/<id>.connector.toml` carries an `[[events]]` and a `[[channels]]` block: for each
      binding its transport, the events it carries, the verification parameters, the discriminator and
      delivery id, the payload map, and the reply as a **rendered oip**.
- [x] The verification block names the **credential**, never a value.
      `crates/connector-cli/tests/site_catalog.rs::no_credential_value_reaches_the_document` must stay
      green with a sentinel set for the signing secret too.
- [x] `catalog.json` carries both, under the existing every-key-always-present rule
      ([catalog-json.md](../designs/catalog-json.md)). Additive, so no `SCHEMA_VERSION` bump.
- [x] A binding whose `verification = "none"` is published **loudly** — a consumer must be able to
      tell a deliberately-unverifiable surface from a verified one without inspecting the absence of a
      field.
- [x] **Nothing reaches the `.flux` module.** A test asserts every shipped module is byte-identical
      across this story, and the emitter *refuses* rather than degrades if asked to emit a binding —
      the tempting wrong output is an event dressed up as a pollable op, which is exactly what
      `AGENTS.md` forbids.
- [x] `--service <name>` selects that service's events and bindings along with its operations, per
      [C-66](C-66-members-under-services.md)'s acceptance. A selection that silently stayed
      operations-only would be the worst kind of partial success.
- [x] The public site renders a provider's inbound surface, or the story records why it does not yet.

## Progress
- The IR half landed 2026-07-30 with [C-82](C-82-channel-bindings-epic.md).
- **Codegen landed.** Events and bindings now reach both publishing backends and neither emitted
  module.

### The verification projection is total, and that is the design decision here

`ChannelBinding::verification` is a tri-state, and publishing it by *omission* would have collapsed
the two cases that must never read alike — "nothing arrives unsolicited over this socket" and "anyone
can POST to this endpoint and we cannot prove otherwise". So every published binding carries a
verification block, that block always names its `kind`, and it restates the one boolean a consumer
filters on:

| IR | `kind` | `verified` |
|---|---|---|
| `Some(Hmac(..))` | `hmac` | `true` |
| `Some(None)` — explicit `verification = "none"` | `none` | **`false`** |
| unset (socket / poll) | `connection` | `true` |

`verified` is `kind != "none"` restated, exactly as `status.works` restates `issues.is_empty()`. An
unset **webhook** — a loader error, so unreachable — maps to `none`/`false` rather than borrowing the
socket's answer, so the arm cannot launder one if the loader rule ever moved.

The classification lives in `crates/connector-cli/src/inbound.rs` and **both** backends read it, for
the reason `catalog.rs` and `site.rs` already share the credential and host walks: two answers to
"is this surface verified?" would be one too many.
`site_catalog.rs::a_deliberately_unverifiable_binding_is_published_loudly` asserts the two blocks
carry the **same key set** and differ only in a value, which is what makes the distinction impossible
to miss by omission.

### Two event fields deliberately stop at the catalogue

The manifest's `[[events]]` carries an event's `name`, `oip`, `description`, `default` and `group`.
It does **not** carry `schema` or `when`. TOML has no `null`, so carrying a vendor JSON Schema
verbatim would make an entirely legal provider file fail to build, over a value the manifest does not
need. `web/public/catalog.json` is the JSON-shaped backend over the same IR and carries both
losslessly — so **a host routing on `when` reads the catalogue, not the manifest.** No shipped
provider declares `when` today; C-84's flux-side design is what should decide its manifest spelling,
and fixing a shape before anything exercises it would be the wrong order.

### `web/public/catalog.json` is not in this commit

It is a whole-catalogue artifact and therefore coordinator-owned (C-104, `AGENTS.md`). This story
changes the *emitter*, so a full build restates the document for all 17 providers; the commit leaves
it alone and the coordinator regenerates at integration. Three tests are red on the branch as a
result, all three reporting only `web/public/catalog.json`:
`catalog_artifacts::the_committed_tree_is_a_fixed_point_of_a_build`,
`readme_snippet::a_build_plans_both_readme_images_and_they_are_current`, and
`site_catalog::the_build_writes_and_checks_site_catalog_json` — the documented shape for a change
that touches an existing provider's artifacts. Verified green with the document regenerated, then
reverted.

The site is in the same position and for the same reason: `web/data/catalog.data.mts` reads
`catalog.json` at build time, so `npm run build && npm test` is red on the branch and green with the
regenerated document (28/28, including the three new checks). The components are written against the
every-key-always-present contract — `provider.channels`, never `provider.channels ?? []` — because a
defensive default is how a contract quietly stops being one.

### Where the assertions live

- `crates/connector-cli/tests/inbound_artifacts.rs` — the manifest half, and the two halves of the
  strict split: `no_event_or_binding_reaches_any_shipped_module` emits each connector **with** and
  **without** its inbound half and asserts the modules, the per-operation renderings and the Rust
  catalogue table are byte-identical (plus the complement, that the manifest *does* differ, so the
  comparison is not vacuous); `a_rendering_for_an_event_or_a_binding_is_refused_by_name` asserts the
  named refusal in both catalogue backends.
- `crates/connector-cli/tests/site_catalog.rs` — the published-document half.
- `web/test/explorer.test.mjs` — the rendering, and the selectors.

## Notes
- Same strict split [C-61](C-61-inbound-codegen.md) states for events; this story supersedes its
  manifest half for bindings and should be read alongside it.
- `crates/connector-cli/src/catalog.rs` and `src/site.rs` share the credential and host walks
  deliberately, so a site and a `cargo add` consumer cannot be told different things. Keep that.
