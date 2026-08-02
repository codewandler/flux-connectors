---
id: C-158
title: "The site's TypeScript catalogue types are a third hand-enumeration with nothing holding them"
pillar: Codegen
status: done
priority: 3
areas: [web, connector-cli]
note: "DONE — the catalogue/type agreement test caught three existing omissions when it landed and caught Provider.config_choices again before v0.12.1; C-453 also put its Node gate in the release transaction"
---

# The site's TypeScript catalogue types are a third hand-enumeration with nothing holding them

## Goal

Make the site's view of `catalog.json` fail when it drifts from the document, the way the Rust
projections now do.

## What was measured

[C-151](C-151-hmac-fields-reach-the-manifest.md) fixed exactly this class on the Rust side: two
projections — `ManifestHmac` and `HmacEntry` — restated `HmacSpec`'s fields by hand and silently
dropped `timestamp_format`. It closed it by **deriving** the authoritative list from
`connector_spec::provider::accepted_keys()`, which reads the field names out of
`deny_unknown_fields`' own error, so a new field fails a test with no test edit.

**`web/data/catalog.mts` is the same shape a third time, one language out.** Its `Hmac`,
`Verification`, `Channel`, `Operation`, `Provider` and `Service` interfaces restate the document's keys
by hand — and **nothing in the Rust gate fails if a TypeScript interface omits a published key.**
C-151 updated it by hand; nothing held it there, and nothing will hold the next one.

The failure is quiet in the worst way: the site still builds, its 28 tests still pass, and a component
simply never reads a key that exists in the document.

## Acceptance

- [x] A test in `web/test/` asserts the **document's own key sets** against the declared interfaces —
      for `Provider`, `Service`, `Operation`, `Channel`, `InboundEvent`, `Verification`, `Hmac` and
      `Reply` at minimum. It reads `web/public/catalog.json`, which the site already ships, so it needs
      no new input.
- [x] **Failing-first test:** delete one field from one interface and the test must name it. Demonstrate
      that, don't assert it.
- [x] It must catch a key the document has and the interface lacks. Whether it *also* catches the
      reverse — an interface field the document never publishes — is a judgement call: the
      every-key-always-present rule ([catalog-json.md](../designs/catalog-json.md)) says the document is
      total, so both directions should hold. Say which you implemented and why.
- [x] The check runs in the **existing** Node gate (`cd web && npm run build && npm test`), not as a new
      tool. The site has exactly one dependency and this should add none.
- [x] It does not name a provider, an operation or an issue code — the hand-maintained-data guard
      forbids that, and it has already caught two false positives from ordinary English (`drives`,
      `notion`). Read the document; do not hardcode a shape.

## Notes

- **The general lesson is worth stating once and applying everywhere:** a type restated by hand in a
  second place will drift, and the drift is invisible because both sides still compile. This repo has
  now hit it three times — C-125 (two derivations of one input schema, resolved with an agreement test),
  C-151 (two projections of one `HmacSpec`, resolved by deriving the list), and here. Each time the fix
  was mechanical once someone measured it.
- **Prefer deriving over asserting** if a path exists. Generating the TypeScript types from the same
  place the document's shape comes from would end the class rather than test it — but that is a bigger
  change, and it means the build writes into `web/`, which is a whole-catalogue artifact question. An
  agreement test is the honest cheap version; say in Progress whether generation is worth a follow-up.
- Also open and adjacent, from C-151: the site does not *render* `timestamp_format` anywhere —
  `InboundSurface.vue` shows the verification chip only. Publishing it was C-151's job; showing it is a
  UI decision nobody has made. Not this story unless you want it.
- One more from the same report: `every_shipped_event_and_binding_reaches_its_manifest` and C-151's
  round-trip both read the **default-service** manifest, so the first multi-service provider with an
  inbound surface will panic in both rather than being silently uncovered. Pre-existing and loud, which
  is the right failure mode — but worth knowing before that provider lands.

## Progress

**Done.** `web/test/catalog_types.test.mjs` — two tests, added to the existing Node gate (40 → 42).
It parses the `export interface` declarations out of `web/data/catalog.mts` as text and walks
`web/public/catalog.json` against them.

**It found three real drifts already on `main`**, which is the answer to whether the class was
theoretical:

- `Provider.runtime` (C-405) — the field a consumer reads to refuse a locally-executing connector.
- `Operation.repeatable_because` (C-186) — the stated condition under which repeating a write is
  safe. 9 of 678 operations publish `idempotency: 'conditional'` and all 9 carry a condition
  (counted from `web/public/catalog.json` in this session); the site could read none of them.
- `Operation.input_schema` (C-125) — the composed calling view, the one object a caller passes.

All three were published by the emitter and declared nowhere in the site's types. Each is now
declared, with the emitter's own reasoning carried across.

**Both directions, and why.** A key the document publishes and the types lack is the drift the story
is about. The reverse — a non-optional declared field the document never carries — is checked too,
because [catalog-json.md](../designs/catalog-json.md) guarantee 1 makes the document *total*: every
key always present, an absent value written as `null` or `[]`. Under a total document a declared
field with no key is the same defect read from the other end. A field written `?` is exempt from that
second direction and only from it, since `?` **is** the declaration that the key may be absent
(`ToolSpec.group` is the only one today).

`Published<T>` is deliberately **not** an exemption. It is a statement about a thinner third-party
source; `public/catalog.json` is the total one, and holding it to publishing every `Published` field
is what would notice the emitter quietly dropping a key components now branch on. C-408's
distinction is untouched — no `published()` call site changed, and no field moved into or out of
`Published`.

**How it finds what to check — no table of paths.** A table of `providers[].operations[]` walks
would be a fourth hand-maintained restatement of the same shape with the same failure mode. The
interfaces already encode containment (`Catalog.providers: Provider[]`,
`Channel.verification: Verification`), so the walk starts at `Catalog` and the document root and
descends wherever a declared field's type names another interface. An interface added to that graph
is covered with no edit here — the property C-151 bought on the Rust side. Array-ness, `| null` and
`Published<…>` are ignored when resolving a target: the document's own value decides how to descend.
A vacuity guard asserts the walk actually reached each of the eight entities the Acceptance named,
so a green run cannot mean "compared nothing".

**No parser dependency.** The declarations are read as text — comments stripped string-aware, then
balanced braces and one field regex. The grammar actually needed is
`export interface X extends Y { field?: Type }`. Same trade `ci_gate.test.mjs` makes for YAML and
`SchemaBlock.vue` for syntax highlighting; the site still has exactly one dependency. The second test
pins the mechanism in the small — both directions, the `?` exemption, a nested entity reached through
the type graph, and four assertions that the real file still parses (prose, a JSDoc `{@link}` brace,
`extends` flattening, an inline object type).

**Is generation worth a follow-up? Yes, but it is a pipeline story, not a test story.** Generating
`catalog.mts` from `site.rs` would end the class rather than test it. Two things make it bigger than
it looks. The generator would write into `web/`, which makes the types a **whole-catalogue artifact**
under `catalog-json.md`'s rule — full-build-only, coordinator-owned, off-limits to a scoped provider
run — so it changes who may run what. And the file is not derivable: `Published<T>`, the site's own
vocabulary (`UNPUBLISHED`, `RISK_ORDER`, `SORTS`, `View`), and the prose on every field live here and
have no source in the emitter. A realistic version generates only the entity interfaces into a
separate emitted module that `catalog.mts` re-exports and refines. Worth filing; this check is what
makes it safe to defer.

**Adjacent, not fixed.** The gate is red at the merge base for an unrelated reason: the
hand-maintained-data guard flags `user` in `OperationDetail.vue` (`· user from`, `· user suffix` —
template prose, and a catalogue value spelled the same). Same two tests fail identically at
`a16a868` with an empty tree, and this diff neither causes nor changes it. It is the third instance
of the C-205 false-positive class after `drives` and `notion`, and the tolerance covers comments but
not template text. Also still open, from C-151: nothing renders `timestamp_format`, and nothing
renders `runtime` either — both are now declared and shown nowhere, which is a UI decision rather
than a type one.
