---
id: C-158
title: "The site's TypeScript catalogue types are a third hand-enumeration with nothing holding them"
pillar: Codegen
status: ready
priority: 3
areas: [web, connector-cli]
note: "found by C-151, which had just derived the Rust side's field list from deny_unknown_fields' own error. web/data/catalog.mts restates the same shape a third time in another language, and NOTHING in the gate fails if it omits a published key"
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

- [ ] A test in `web/test/` asserts the **document's own key sets** against the declared interfaces —
      for `Provider`, `Service`, `Operation`, `Channel`, `InboundEvent`, `Verification`, `Hmac` and
      `Reply` at minimum. It reads `web/public/catalog.json`, which the site already ships, so it needs
      no new input.
- [ ] **Failing-first test:** delete one field from one interface and the test must name it. Demonstrate
      that, don't assert it.
- [ ] It must catch a key the document has and the interface lacks. Whether it *also* catches the
      reverse — an interface field the document never publishes — is a judgement call: the
      every-key-always-present rule ([catalog-json.md](../designs/catalog-json.md)) says the document is
      total, so both directions should hold. Say which you implemented and why.
- [ ] The check runs in the **existing** Node gate (`cd web && npm run build && npm test`), not as a new
      tool. The site has exactly one dependency and this should add none.
- [ ] It does not name a provider, an operation or an issue code — the hand-maintained-data guard
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
