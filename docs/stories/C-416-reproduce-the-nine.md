---
id: C-416
title: "Reproduce babelforce's nine operations through the spec route, byte-identical"
pillar: Spec
status: in-progress
priority: 3
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
- [x] `providers/babelforce.toml` points at the vendored documents and selects exactly the nine
      operations it ships today, with their existing ids unchanged.
      → `providers/babelforce.toml:102-106` pins the manager document; nine `[[patch.operations]]`
      blocks select it. Proven by `crates/connector-spec/tests/babelforce_spec_route.rs`, which
      asserts the nine `(id, method, path)` triples and that selection names exactly nine.
- [ ] `connectors/babelforce.flux` and `connectors/babelforce.connector.toml` are **unchanged** —
      `cargo run -p connector-cli -- diff` reports them up to date with no regeneration, and the
      `ir_sha256` in `connectors.lock` is the same value.
      → **Deliberately not satisfied, and it cannot be.** `connectors/babelforce.flux` changed;
      `connectors/babelforce.connector.toml` is byte-identical. The decision and its causes are in
      Progress below — the largest is that the document publishes response schemas for all nine
      where the hand-authored file had none, and keeping a hash stable would have meant deleting
      them. (`connectors.lock` does not exist in this repository, so the `ir_sha256` half of this
      bullet has nothing to compare.) `cargo run -p connector-cli -- build` regenerates cleanly and
      `diff` then reports **557 artifacts up to date (53 providers checked)**.
- [x] Every deliberate departure from the document survives the conversion, each still carrying the
      comment that explains it: the production `base_url` (the document's `servers[0]` is staging),
      the excluded `X-Auth-Access-*` pair, and the refusal to implement the OAuth password grant.
      → `providers/babelforce.toml:46-64` (`base_url`, with its premise corrected — see Progress),
      `:118-150` (the header pair), `:152-164` (the password grant). All three survive; two of the
      three needed their stated *reason* rewritten because the vendored bytes disagree with it.
- [x] The excluded header pair is handled honestly. **This bullet's premise changed on 2026-08-01**:
      C-415 measured the vendored documents and `X-Auth-Access-Id`/`X-Auth-Access-Token` are **not
      declared in any of the five** — `securitySchemes` holds `oauth2` alone. So there is nothing for
      an overlay `auth` to remove and nothing for drift-check to report on, and
      `providers/babelforce.toml:88-96`'s instruction ("ingest must keep *seeing* the pair") is not
      satisfiable against this spec version. Either the maintainers finished the scrubbing the
      inventory said was under way, or these documents were never the ones that declared it. Confirm
      which with the API owners, then rewrite that comment block to say what is true.
      → Confirmed independently at `manager-2026-07-10.openapi.yaml:26270-26279`: `securitySchemes`
      holds `oauth2` alone. The block is rewritten (`providers/babelforce.toml:118-146`) to say the
      pair is *not declared* (`:118-150`), to record that no per-operation `auth` is therefore
      written, and to keep it refused if a future pull re-declares it. **The question for the API owners
      is still open** and is named in the file and in Progress below.
- [x] The `SCHEMA GAP:` comment at `providers/babelforce.toml:17` is deleted: provenance is now
      reachable, which was the whole reason it was written.
      → Deleted. `[spec] sha256` is checked against the ingested bytes by `load_with_spec`, and
      `babelforce_spec_route.rs::the_documents_identity_is_recorded_and_checked` asserts it.
- [x] Where the document and the hand-authored file disagree, the diff is recorded in this story
      before it is resolved. A silent correction here is the one outcome that would waste the test.
      → Four disagreements found and recorded in Progress before resolution. None was corrected
      silently; each is also carried in a comment at its own `[[patch.operations]]` block.

## Progress

**2026-08-01 — converted, measured, blocked, unblocked, and complete.** The nine operations
reproduce through the spec route. The conversion surfaced two capability gaps that no story in the
epic had anticipated; both were filed, both landed (C-421, C-422), and this file now uses them. The
sections below are kept in the order they were learned rather than rewritten into hindsight, because
the order is the finding: **everything expensive about this conversion was invisible until a real
vendor document met a real shipped connector.**

One decision remains unverified and is flagged rather than buried — the request body of
`babelforce-call-session-set`, under (a) below. Its offline evidence was re-examined and got weaker,
not stronger.

### The cost, as a number (C-6's re-cut acceptance asks for exactly this)

Whole file: **533 → 420 lines**. Excluding comments and blanks — the declarations a reader must
actually maintain — **306 → 98 lines**. Restricted to the operation blocks:

| operation | hand-authored | patch | |
|---|---:|---:|---|
| `babelforce-agent-list` | 79 | 8 | 11 query params, all redeclared by the document |
| `babelforce-agent-get` | 12 | 5 | |
| `babelforce-agent-status-update` | 23 | 6 | |
| `babelforce-call-list` | 115 | 34 | 26 of the 34 are the `omit.query` list, one name per line |
| `babelforce-call-get` | 12 | 5 | |
| `babelforce-call-hangup` | 12 | 5 | |
| `babelforce-call-session-set` | 14 | 6 | |
| `babelforce-session-get` | 12 | 5 | |
| `babelforce-session-update` | 14 | 6 | |
| **total** | **293** | **80** | **32.6 → 8.9 lines per operation** |

**So the epic's cost bet is won, decisively and with room to spare.** A patch block is 5 lines when
the document is right and 6–8 when it needs a sentence, against a mean of 33 hand-authored. The
5-line floor is `select`/`rename`/`risk`/`idempotency` plus the block header, and three of those four
are the fields no specification carries — so C-411's selectors and C-412's naming rule attack exactly
the part that dominates at scale, and the projection for 397 improves rather than degrades.

**The one expensive block is expensive in the right way.** `babelforce-call-list` costs 34 lines
against 115, and 26 of those are one vendor parameter name per line — a list a reviewer reads down,
not logic. It is the only operation of the nine where the document needed real correcting, and it
still came in at under a third of the hand-authored cost. Note the shape of the saving: the
hand-authored file paid ~5 lines to *describe* each of 14 kept parameters, and this pays ~1 line to
*name* each of 24 dropped ones. Curation by exclusion is cheaper than curation by transcription
whenever a vendor documents more than half of what you want, which for a 356-operation document is
always.

**Zero parameter patches were needed.** This is the result that most surprised me and it is worth
stating on its own: not one of the nine operations needed a `[[patch.operations.params]]` block.
The vendored document's parameter descriptions and JSON Schemas are *better* than the hand
transcription they replace — it types `state`, `type`, `finishReason` and `id` as `oneOf`
scalar-or-array where the hand-authored file had flattened them to plain scalars, and its
descriptions are fuller ("When specified searches multiple fields at once: name, group.name,
number, email, sourceId, integration.label" against "Free-text search over agent name and group
name"). The overlay's parameter-correction machinery, which C-6 treats as its centre of gravity,
went entirely unused against a real vendor document.

### Byte-identity or the new schemas: **the new schemas win, deliberately**

`connectors/babelforce.flux` **changed** (+135 lines); `connectors/babelforce.connector.toml` did
not. Byte-identity was reachable only by throwing away real vendor-published information, so it was
refused. Four independent causes, in descending order of how much I would defend them:

1. **Response schemas now exist: `babelforce 0/9 → 9/9`.** The document publishes a 2xx JSON schema
   for all nine, and ingest derives them. The old file argued at length (its own §"NO OPERATION HERE
   DECLARES A `response_schema`") that no offline source existed to derive one from, and named this
   exact route as what would unblock it. Keeping byte-identity would have meant deleting nine
   derived schemas to protect a hash. This is the single largest artifact change and the clearest
   win.
2. **Better parameter schemas and descriptions**, as above. Also a win.
3. **`babelforce-agent-list` gains a twelfth query parameter, `tags`.** The hand transcription
   predates it. A real parameter, correctly ingested.
4. **`babelforce-call-list` gains 24 query parameters it does not want.** This one is a
   **regression** and it is the conversion's real cost — see below.

### The four disagreements, recorded before resolution

**(a) `babelforce-call-session-set`'s request body — a genuine wire-level conflict.** The
hand-authored file declared `body_schema = {type = "object"}` (the body *is* a free-form map, so it
sends `{"app.priority": "high"}`), citing inventory §6.5's reading of the older `0.7.0` document, in
which `SetCallSessionVariablesRequest` had no `properties`. The vendored 2026-07-10 document
declares that same schema name as a **wrapper** with one property, `variables`
(`manager-2026-07-10.openapi.yaml:23376-23390`), so the body is
`{"variables": {"app.priority": "high"}}`. These are different requests and at most one works.
**The document is taken.** It is newer by fifteen months; it is internally consistent (the
operation's own `requestBody.example` shows the wrapper, and the paired response schema is new in
the same document); and the hand-authored reading descends from a *missing* `properties` key, which
is what an under-specified schema and a genuine free-form map look like identically — a schema that
gained a property is far more often one that was completed than an API that changed shape.
**What would settle it: one live call.** Send both bodies to a test account and see which sets the
variable. Until then this is a considered choice, not a verified fact.

> **Re-examined 2026-08-01 on the coordinator's request, and my confidence went down.** I went
> looking for anything else offline that bears on it. There is, and it mostly cuts the other way.
>
> **Every other session-variable payload in the manager document is a bare, unwrapped map — five of
> them.** `UpdateSessionVariablesRequest` (`:25854`, the sibling write this connector also ships);
> `ConversationSessionVariables` (`:14687`), whose `PUT` says in prose "**The body is a key/value
> map** merged into the session's user scope"; the inline `session` map on the test-call request
> (`:11578`, "Session variables to set on the call"); the named component `SessionVariables`
> (`:23369`); and `SessionResponse_item.data` (`:23367`), the read side, "session variables as
> key-value pairs". `SetCallSessionVariablesRequest` is the **sole** wrapper among six payloads for
> one concept in one document.
>
> That a bare `SessionVariables` component already exists and this operation does **not** `$ref` it
> is the sharpest form of the question: either someone deliberately modelled this one endpoint
> differently, or someone completing an under-specified schema invented a wrapper and did not check
> the neighbours.
>
> Two things still argue for the wrapper, and they are not nothing: the wrapper is asserted three
> times in that operation's block (the `description`, the `requestBody.description`, and a concrete
> `example`), and a *positive* assertion of a property is stronger evidence than the *absence* of a
> `properties` key that the old reading rests on. But those three are **not independent** — one
> author wrote them together. And that same block is demonstrably loose in one place: its prose says
> the endpoint returns `{ success, item: { id } }` while its own `SetCallSessionVariablesResponse`
> (`:23391`) declares `success` alone.
>
> **Revised position: roughly even, where I previously had it near 70/30 for the wrapper.** I am not
> reversing the choice on this evidence — reversing on a document reading is exactly the move that
> got us here — but the epistemic status has changed and the story should say so rather than let the
> original confident paragraph stand.
>
> **Both failure modes are silent, which is the part that should decide the priority.** The vendor
> ignores keys not prefixed `app.`, so a wrapper sent to a bare-map endpoint has its lone
> `variables` key ignored, and a bare map sent to a wrapper endpoint has all its keys ignored. Either
> way the write is a **no-op that returns `200 {"success": true}`** — no error, no data corruption,
> and nothing a caller or a test would notice. That is why this cannot be left to surface on its own.
>
> **The asymmetry that matters for the decision:** the flat shape is what ships today. Changing it on
> documentary evidence alone risks *introducing* a regression into something that may work, whereas
> leaving it risks preserving a pre-existing bug. Those are not equally bad defaults. If the owner
> cannot answer quickly, **reverting this one operation to `body_schema` (the flat map) is the
> conservative choice** and costs three lines; the rest of the conversion does not depend on it.

**(b) The method conflict named in the dispatch does not exist.** The dispatch stated that
`providers/babelforce.toml` declares `babelforce-call-session-set` as **POST** while the document
declares **PUT**. Both sides are **PUT**: the hand-authored file said `method = "PUT"` at its
line 477, and the document declares `put:` at `manager-2026-07-10.openapi.yaml:3503`. There was
nothing to reconcile. Recorded because the instruction to reconcile it was emphatic, and a future
reader should not go looking for a resolution that was never needed.

**(c) `servers[0]` is no longer staging.** The `base_url` comment's stated reason — "the document's
`servers[0]` is **staging** (`https://latest.dev.babelforce.com`), so a positional take-servers[0]
ingest would silently point the connector at the dev environment" — is false of the vendored bytes.
C-415's pull normalizes `servers:` to the public production host, and the document now declares one
server, `https://services.babelforce.com`, described as `Production`
(`manager-2026-07-10.openapi.yaml:26283-26285`). The staging URL appears **zero** times across all
five documents. `base_url` stays declared anyway and the comment now says why for a reason that is
still true (agreeing with the document today is not deferring to it); but the old sentence would
have been a false claim in a shipped file.

**(d) `from`/`to` versus `fromNumber`/`toNumber` — the document settles an open question, against
us.** The hand-authored file selected `fromNumber`/`toNumber` and flagged it "**Unconfirmed — worth
one question to the API owner**". The document answers it in the opposite direction: it documents
`fromNumber` as "Alias of `from`" and `toNumber` as "Alias of `to`", making the *short* pair
canonical. Both pairs now travel (nothing can drop either), so this is recorded in the operation's
description rather than in the parameter set.

### The regression that was found, filed and closed inside one story

The hand-authored block selected 14 of the document's query parameters and dropped 24 by hand, with
a paragraph justifying each drop — **18** `filters.`-prefixed parameters that are *exact* synonyms
of their flat forms ("almost certainly one serializer emitting both bindings"), plus `domain`,
`source`, `anonymous`, `parentId` as too narrow, plus one of each aliased number pair.

**When first measured, `ParamPatch` could not drop a parameter.** It corrected `required`,
`description` and `schema` on a parameter the document declares and refused a name it does not, so
all 38 travelled and this operation reached a model as a 38-argument tool of which 18 arguments
were exact duplicates of 18 others. That was the one place hand-authoring beat patching.

**C-422 closed it and this file now uses it.** `omit.query` at
`providers/babelforce.toml:283-308` drops the same 24 by name, and the operation is back to the
curated 14 the hand-authored file shipped. The cost is **24 names plus one key against the 24
parameter blocks it replaces**, so the capability arrived without spending the cost win — and every
omission is a written-down decision the loader refuses when it stops matching, rather than a
similarity someone inferred.

Worth keeping for the epic's record: the gap was **found by conversion, not by design review**. It
is the opposite direction from C-6's open "a patch can add a parameter the vendor omits" bullet, it
was named nowhere in the epic beforehand, and against a 356-operation document with duplicated
filter bindings it is the more load-bearing of the two.

**Correction to my own earlier count.** I first reported 17 `filters.` synonyms; it is **18**
(`filters.` restates 18 of the 20 flat parameters — `page` and `max` have no prefixed twin). C-422's
story note carries the 17 from my handoff and should be read as 18. The omit list itself was always
24 names and is unaffected.

### The blocker — filed as C-421, fixed, and now cleared

*Resolved 2026-08-01. Kept in full because it is the epic's most transferable finding: it is what
C-417 and C-420 would each have hit, and the shape of the fix is why they now will not.*

This is the finding that stopped integration, it is not specific to babelforce, and it was not
anticipated by any story in the epic.

`provider::load` takes bytes and no spec cache. Called on a spec-backed provider it yields the
documented *skeleton* — id, base URL, credentials, provenance, and **zero operations**. Every test
in this repository that reads `providers/*.toml` calls plain `provider::load`: **82 files under
`crates/` do**, and that convention was correct for exactly as long as all 53 providers were
hand-authored. Babelforce becoming the first `[spec]` provider falsifies it repo-wide, in one commit.

Measured on this branch: **53 tests across 18 binaries in 4 crates** go red. They are not eight
whole-catalogue staleness failures that a coordinator regenerates; only **4** of the 53 are that
(`a_build_plans_both_readme_images_and_they_are_current`,
`the_build_writes_and_checks_site_catalog_json`, `the_committed_tree_is_a_fixed_point_of_a_build`,
`the_published_catalogue_carries_the_service` — all downstream of the one stale
`web/public/catalog.json`).

The proximate cause is concentrated, which is the good news. **38 of the 53** fail at one line —
`validate_verify` refusing `verify = "babelforce-agent-list"` because the skeleton declares no
operations. I ran the experiment (skip that check when `spec.is_some() && ingested.is_none()`) and
**reverted it**: it cuts the failures from 53 to 15, but the residual proves it is the wrong fix —
`every_shipped_provider_loads` then fails with "declares no operations, so it compiles to an empty
module", which is the test being *right*. The tests do not need the refusal lifted; they need the
spec cache.

So the prerequisite is real work with a design question in it, and it belongs in its own story:

- **Decide what plain `load` should do with a spec-backed file.** Silently returning a
  zero-operation skeleton that validates is the dangerous option — it makes catalogue-wide tests
  pass *vacuously* over a provider they think they checked. Refusing outright ("this file is
  spec-backed; load it with `load_with_spec` and its cache") is louder and probably right, but it is
  a semantic change to a published crate's loader and no story sanctions it.
- **Give the test suite one spec-aware way to load a shipped provider.** There is no shared
  provider-loading helper in `crates/*/tests` today; each of the 18 binaries has its own. `AGENTS.md`
  is explicit that `connector-spec` does no filesystem IO, so the cache has to be assembled by the
  caller.
- **Re-baseline two figures C-126 owns**: `the_recorded_floor_is_the_measured_figure` and
  `the_recorded_ceiling_is_the_measured_absence` both move when babelforce goes 0/9 → 9/9.
  Coordinator-owned by `AGENTS.md`'s fence, so not this story's to touch.
- **Rewrite three babelforce assertions that encode the hand-authored shapes**:
  `babelforce_nests_the_presence_label` and `babelforce_sends_its_free_form_session_bodies` (I
  reported these as living in `connector-flux/tests/babelforce_ivr.rs`; they are in
  `crates/connector-cli/tests/shipped_providers_build.rs:245-352` — C-421 corrected me), and
  `connector-pack/tests/request.rs::a_free_form_body_travels_whole_in_either_spelling`. The last one
  only needs repointing at `babelforce-session-update`, which stays free-form; the other two assert
  facts this conversion deliberately changed (see (a) above and `presence` below).

**How it was resolved (C-421, merged at `0bdaee3`).** Plain `provider::load` now *refuses* a
spec-backed file with a message naming `load_with_spec` — the louder of the two options above, and
the right one. The load-bearing half is a shared test seam,
`crates/connector-spec/tests/support/shipped_provider.rs`, which reads a provider's definition plus
**every** document under `specs/<name>/` and lets the loader resolve the pins; 72 call sites in 66
files moved onto it. The design goal was that a conversion should then cost no test changes at all,
and measured against this branch it very nearly held: **one** call site was missed —
`response_schema_coverage.rs` kept a local `load()` on the pure loader — and moving it is the only
test edit this conversion needed. That is a good result for C-417 and C-420, which convert the rest
of the catalogue, and it is worth noting the miss was in the one file whose helper is *not* named
`load`-and-forget: it is the ratcheted measurement, so a coverage figure measured through the pure
loader would have counted babelforce's nine as absent forever rather than failing loudly.

### Smaller notes

- **`presence.name` flattening is lost.** The hand-authored file declared a scalar `presence_name`
  carrying `wire = "presence.name"` (C-29); ingest expands only the *top* level of a request body by
  design, so the document's `presence` arrives as one object-typed parameter. Both reach the wire as
  `{"presence": {"name": …}}`, so this is an ergonomic change for the caller and not a wire change.
  `ParamPatch` carries no `wire`, so it cannot be restored by an overlay.
- **Ingest earns 5 diagnostics on the manager document**, all `multipart/form-data` request bodies,
  all on operations outside the selected nine. That is `docs/designs/spec-front-end.md`'s known
  blocker #1 arriving exactly where it said it would, not a surprise.
- **`connectors/babelforce.connector.toml` is byte-identical.** The manifest carries auth, hosts and
  config, none of which the conversion touched — worth knowing for C-417, since it means the
  operator-facing surface is stable across a front-end change.
- Full `flux-connectors diff` reports **1 artifact would change**: `web/public/catalog.json`, which
  is whole-catalogue and coordinator-owned. The per-provider gate
  (`diff --provider babelforce`) reports 12 artifacts up to date.

## Notes
- **This is the go/no-go for the epic.** `docs/stories/C-6-overlay-layer.md` states the bet: "if
  patching a bad vendor spec turns out harder than hand-writing the integration, the whole premise
  needs revisiting". Nine operations with known-correct output is the cheapest place to find out.
- The nine currently declare no `response_schema` (C-126 records babelforce as the largest absent
  block). The manager document carries a 2xx schema for 352 of its 356 operations, so this conversion
  probably *adds* schemas — which changes the artifacts. Decide deliberately whether byte-identity or
  the new schemas wins, and record which; do not let it happen by accident.
