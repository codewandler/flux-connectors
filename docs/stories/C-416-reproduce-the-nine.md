---
id: C-416
title: "Reproduce babelforce's nine operations through the spec route, byte-identical"
pillar: Spec
status: blocked
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [providers, connector-spec]
note: "the migration safety net and C-6's real test — providers/babelforce.toml:14 has said since C-17 that 'the operation set below is the selection to reproduce'. If the spec route cannot reproduce nine hand-checked operations, it must not be trusted with 397"
---

# Reproduce babelforce's nine operations through the spec route, byte-identical

## Goal
Convert `providers/babelforce.toml` from hand-authored to `[spec]` + patches and produce **the same
artifacts, byte for byte** — the only honest evidence that ingest plus overlay is at least as good as
hand-authoring.

## Acceptance
- [ ] `providers/babelforce.toml` points at the vendored documents and selects exactly the nine
      operations it ships today, with their existing ids unchanged.
- [ ] `connectors/babelforce.flux` and `connectors/babelforce.connector.toml` are **unchanged** —
      `cargo run -p connector-cli -- diff` reports them up to date with no regeneration, and the
      `ir_sha256` in `connectors.lock` is the same value.
- [ ] Every deliberate departure from the document survives the conversion, each still carrying the
      comment that explains it: the production `base_url` (the document's `servers[0]` is staging),
      the excluded `X-Auth-Access-*` pair, and the refusal to implement the OAuth password grant.
- [ ] The excluded header pair is handled honestly. **This bullet's premise changed on 2026-08-01**:
      C-415 measured the vendored documents and `X-Auth-Access-Id`/`X-Auth-Access-Token` are **not
      declared in any of the five** — `securitySchemes` holds `oauth2` alone. So there is nothing for
      an overlay `auth` to remove and nothing for drift-check to report on, and
      `providers/babelforce.toml:88-96`'s instruction ("ingest must keep *seeing* the pair") is not
      satisfiable against this spec version. Either the maintainers finished the scrubbing the
      inventory said was under way, or these documents were never the ones that declared it. Confirm
      which with the API owners, then rewrite that comment block to say what is true.
- [ ] The `SCHEMA GAP:` comment at `providers/babelforce.toml:17` is deleted: provenance is now
      reachable, which was the whole reason it was written.
- [ ] Where the document and the hand-authored file disagree, the diff is recorded in this story
      before it is resolved. A silent correction here is the one outcome that would waste the test.

## Progress
- 2026-08-01 — **Implemented and BLOCKED, not abandoned.** The conversion itself is done and lives on
  branch `impl/C-416`: `providers/babelforce.toml` points at the vendored manager document and selects
  the nine. **The cost bet is won decisively** — operation blocks went 293 → 54 declaration lines
  (32.6 → 6.0 per operation), whole file 533 → 364, with **zero parameter patches needed**. That is
  C-6's central question answered with a number.
- **Blocked on [C-421](C-421-loading-a-provider-is-not-spec-aware.md)**, which the conversion exposed:
  `provider::load` takes no spec cache, so a spec-backed provider loads as a zero-operation skeleton
  for the 86 test files that call it directly. 53 tests across 18 binaries go red the moment one
  shipped provider converts. The branch is the reproduction; do not delete it.
- **Byte-identity was deliberately refused**, as this story's Notes invited. `connectors/babelforce.flux`
  grows 135 lines because the document publishes response schemas for all nine operations where the
  hand-authored file had none — the exact gap C-126 recorded babelforce as the largest block of.
  `connectors/babelforce.connector.toml` is byte-identical.
- **Four disagreements found, recorded before resolution.** The real one is the request body of
  `babelforce-call-session-set`: hand-authored sends `{"app.priority": "high"}`, the document declares
  a wrapper `{"variables": {…}}`. The document was taken — newer by 15 months, internally consistent
  with its own example and response, and the old reading descends from a *missing* `properties` key.
  **One live call would settle it; nothing offline can.** Reversible in one line.
- **Two premises this story shipped with were false, and the implementor disproved both.** There is no
  POST/PUT conflict on `babelforce-call-session-set` — the file says `PUT` (line 477) and so does the
  document; the coordinator asserted otherwise from a hand-typed list. And `servers[0]` is Production,
  not staging: C-415's pull normalizes it and the staging URL appears zero times across all five
  documents, so the `base_url` comment's stated reason was wrong and is rewritten.
- **The vendor settles `from`/`to` against us**: it documents `fromNumber`/`toNumber` as aliases *of*
  `from`/`to`, the opposite of the hand-authored guess.

## Notes
- **This is the go/no-go for the epic.** `docs/stories/C-6-overlay-layer.md` states the bet: "if
  patching a bad vendor spec turns out harder than hand-writing the integration, the whole premise
  needs revisiting". Nine operations with known-correct output is the cheapest place to find out.
- The nine currently declare no `response_schema` (C-126 records babelforce as the largest absent
  block). The manager document carries a 2xx schema for 352 of its 356 operations, so this conversion
  probably *adds* schemas — which changes the artifacts. Decide deliberately whether byte-identity or
  the new schemas wins, and record which; do not let it happen by accident.
