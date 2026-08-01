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
      → **Deliberately not satisfied, and it cannot be.** `connectors/babelforce.flux` changed by
      135 lines; `connectors/babelforce.connector.toml` is byte-identical. The decision and its
      four causes are in Progress below. (`connectors.lock` does not exist in this repository, so
      the `ir_sha256` half of this bullet has nothing to compare.)
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

**2026-08-01 — converted, measured, and BLOCKED on a prerequisite nobody had costed.** The
conversion itself works and the nine operations reproduce. It cannot be integrated as it stands,
for a reason that is not about babelforce: see "The blocker" below.

### The cost, as a number (C-6's re-cut acceptance asks for exactly this)

Whole file: **533 → 364 lines**. Excluding comments and blanks — the declarations a reader must
actually maintain — **306 → 72 lines**. Restricted to the operation blocks:

| operation | hand-authored | patch | |
|---|---:|---:|---|
| `babelforce-agent-list` | 79 | 8 | 11 query params, all redeclared by the document |
| `babelforce-agent-get` | 12 | 5 | |
| `babelforce-agent-status-update` | 23 | 6 | |
| `babelforce-call-list` | 115 | 8 | 14 query params, all redeclared by the document |
| `babelforce-call-get` | 12 | 5 | |
| `babelforce-call-hangup` | 12 | 5 | |
| `babelforce-call-session-set` | 14 | 6 | |
| `babelforce-session-get` | 12 | 5 | |
| `babelforce-session-update` | 14 | 6 | |
| **total** | **293** | **54** | **32.6 → 6.0 lines per operation** |

**So the epic's cost bet is won, decisively and with room to spare.** A patch block is 5 lines when
the document is right and 8 when it is not, against a mean of 33 hand-authored — and the ratio
*improves* with parameter count, which is the direction that matters for C-417's 397. The 5-line
floor is `select`/`rename`/`risk`/`idempotency` plus the block header, and three of those four are
the fields no specification carries. C-411's selectors and C-412's naming rule attack exactly that
floor, so the projection for 397 operations is better than 6 lines each, not worse.

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
**What would settle it: one live call.** Nothing offline can. Send both bodies to a test account
and see which sets the variable. Until then this is a considered choice, not a verified fact.

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

### The regression: `babelforce-call-list` becomes a 38-argument tool

The hand-authored block selected 14 of the document's query parameters and dropped 24 by hand, with
a paragraph justifying each drop — 17 `filters.`-prefixed parameters that are *exact* synonyms of
their flat forms ("almost certainly one serializer emitting both bindings"), plus `domain`,
`source`, `anonymous`, `parentId` as too narrow, plus one of each aliased number pair.

**`ParamPatch` cannot drop a parameter.** It corrects `required`, `description` and `schema` on a
parameter the document declares, and refuses a name the document does not
(`crates/connector-spec/src/provider.rs::correct`). So all 38 travel. The emitted signature is
visible in `connectors/babelforce.flux` and it is not defensible as a model-facing tool: 38
arguments of which 17 are exact duplicates of 17 others.

This is the one place hand-authoring beat patching, and it is a **capability gap, not a cost
problem** — one `omit`/`drop` list on `OperationPatch` would have cost 4 lines here against the 24
lines of hand-authored parameter blocks it replaces, keeping the cost win intact. **C-6 should grow
parameter exclusion, and it should land before C-417.** Note the asymmetry with C-6's currently open
bullet, which is about *adding* a parameter the vendor omits: this is the opposite direction, it is
not named anywhere in the epic today, and against a 356-operation document with duplicated filter
bindings it is the more load-bearing of the two.

### The blocker: no shipped provider can become spec-backed until the test suite is spec-aware

This is the finding that stops integration, it is not specific to babelforce, and it was not
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
  `connector-flux/tests/babelforce_ivr.rs::babelforce_nests_the_presence_label` and
  `::babelforce_sends_its_free_form_session_bodies`, and
  `connector-pack/tests/request.rs::a_free_form_body_travels_whole_in_either_spelling`. The last one
  only needs repointing at `babelforce-session-update`, which stays free-form; the other two assert
  facts this conversion deliberately changed (see (a) above and `presence` below).

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
