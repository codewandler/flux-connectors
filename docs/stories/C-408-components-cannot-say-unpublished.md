---
id: C-408
title: "The explorer components cannot say 'this source does not publish that', so a thinner catalogue reads as a claim about the connector"
pillar: Surfaces
status: done
note: "found 2026-08-01 by flux-exchange's console, the second consumer C-191/C-238 anticipated. It publishes no auth and no credentials, so ProviderCard renders 'not configured' in the danger colour on every card — a red claim the service never made"
---

# The explorer components cannot say "this source does not publish that"

## Goal

A component rendering a catalogue that omits a field says **"this source does not publish that"**,
not **"the connector does not have that"**. The two are different facts and today they render
identically.

## How this surfaced

flux-exchange's console now fetches a live catalogue from its own service and renders it through the
15 carried components. That service publishes a deliberately thinner document than
`public/catalog.json`: no `auth`, no `credentials`, no `method`/`path`, no `flux`, no `base_url`.

Every one of those absences currently renders as a **statement about the connector**:

| where | what it renders | what it means | what it says |
|---|---|---|---|
| `ProviderCard.vue:73-75` | `auth.schemes.length === 0` → **"not configured"** in `--vp-c-danger-1` | this source publishes no auth | *this connector has no auth* — in red, on every card |
| `OperationDetail.vue:131-133` | empty `credentials` and no `Note` → "No safe credential configuration is available for this operation. Live calls are disabled." | this source publishes no credentials | *this operation cannot be called safely* |
| `OperationDetail.vue:89-90`, `OperationRow.vue:55-56` | `method`/`path` rendered unconditionally | this source publishes no request shape | two empty chips |

The first is the worst: a red "not configured" on every provider card is a false claim about
somebody else's connector, made by a component that had no way to know better.

That consumer worked around it in its own layer — an app-scoped issue notice saying an empty field
means unpublished-here — but the components are where the distinction belongs, because the next
consumer will hit this too. **This is exactly the second-consumer pressure C-191 and C-238 are
about**, arriving as predicted.

## Acceptance

- [x] A component can distinguish **absent** from **not published by this source**, and renders them
      differently. The mechanism is this story's design decision — an `Auth | null` where `null`
      means unpublished, a source-capability descriptor, or something better — but a bare empty
      collection must stop carrying two meanings.
- [x] **Failing-first test** — a catalogue document that omits `auth` does **not** render "not
      configured", and does not render in the danger colour. This is the regression that motivated
      the story.
- [x] Same for `credentials`: an unpublished credential set does not render as "live calls are
      disabled".
- [x] `method`/`path` are omitted rather than rendered as empty chips when unpublished.
- [x] The 15 components still import only Vue, a sibling, and `catalog.mts` — the invariant
      `no_component_imports_the_site_framework` asserts. Whatever carries the distinction arrives as
      a prop or as injected context; **no component learns which source it is rendering.**
- [x] `web/`'s own rendering of the full catalogue is unchanged — this is additive, and a
      byte-identical render of `public/catalog.json` is the proof.

## Also, from the same consumer

- [x] **`Operation` in `catalog.mts` has no `effects`, `effects_derived` or `admitted`.** That is the
      field set flux-exchange's catalogue route exists to publish — `risk` and `idempotency` are
      there, effects are not — so a host serving grant-relevant metadata has to carry it alongside
      the shared type rather than in it. Decide whether the shared `Operation` gains them. Note that
      `effects` there is *derived* (from non-empty `hosts`) rather than declared, so a flag saying so
      travels with it or the field lies.

## Progress

**The mechanism: the document says what it carries, and one predicate reads it.**

`data/catalog.mts` gains `Published<T> = T | null | undefined` and `published(value)`. A field typed
`Published<T>` is one the generated catalogue always publishes and a thinner source may not, and
`published` is the **only** place either spelling of an absence is interpreted — a source that omits
a key and one that writes `null` are saying the same thing, and no consumer should have to know
which its source chose.

Six fields are now `Published`: `Provider.auth`, `Provider.base_url`, `Operation.method`,
`Operation.path`, `Operation.credentials`, `Operation.flux` — exactly the set the consumer reported.
Three accessors give a component a value it can branch on three ways instead of a `.length` that
collapses two of them: `providerAuth`, `operationCredentials`, `searchablePath`; and `signature` now
returns `string | null`.

No component learns which source it is rendering, and none needed a new prop or a new injection: the
distinction is a property of the **document**, so it arrives with the data that was already there.
The import invariant is untouched — the four changed components import Vue, siblings and
`catalog.mts`, and `no_component_imports_the_site_framework` still passes.

**The distinction that is deliberately preserved.** "Withheld, and that is worth showing in red" is a
different fact from "this catalogue does not carry that field", and only the second is new:

- `ProviderCard` renders a three-way Auth fact. `auth && auth.schemes.length` lists the schemes;
  `auth` alone still renders **"not configured"** in `.card__warn`, which is still
  `--vp-c-danger-1`; only the `v-else` — no auth published at all — renders the muted
  `UNPUBLISHED` sentence. The one connector in today's catalogue that publishes an empty scheme list
  reads exactly as it did.
- `OperationDetail`'s "No safe credential configuration … Live calls are disabled." gains one term:
  `credentials && !credentials.length && !clear.length`. All nine operations that withhold a
  credential today still say it; a source publishing no credentials no longer does.
- `method`/`path` are omitted rather than rendered blank, in both `OperationDetail` and
  `OperationRow`.
- `signature()` used to **throw** on an unpublished `flux`, so that absence was not a misleading
  render but a broken page. It returns `null` and the page says so.

**Evidence.** Five tests appended to `web/test/explorer.test.mjs` (35 → 40, all green). Four of them
fail at the merge base; the fifth is the additive half. The additive proof is also mechanical:
rebuilt `.vitepress/dist` is byte-identical to the base build for every one of the 1,241 files except
the stylesheet (two new rules) and the asset-hash references that follow it — no page's rendered text
changes. `cargo run -p connector-cli -- diff` still reports `557 artifacts up to date (53 providers
checked)`; nothing under `crates/` was touched.

**Decision on the second section — `Operation` does not gain `effects`, `effects_derived` or
`admitted`, and here is why.**

1. `catalog.mts` is typed against a document this repository *generates*. `connector-cli build`
   emits no `effects` on an operation, so the field would describe a document no source here
   produces — and typing it `Published` would make it read "not published by this source" for all
   299 operations of the only catalogue the site actually has.
2. The consumer's `effects` is derived from non-empty `hosts`, which the shared `Operation` already
   carries. A consumer can derive it identically today, and `effects_derived` would be a constant
   `true` for the only producer that has one — a flag with one value is not a flag.
3. `admitted` is a grant decision. It is a property of a host's policy and a tenant, not of the
   connector, and it has no honest home on a type whose every other field is the connector's own.
4. Declared-versus-derived is exactly [C-235](C-235-the-catalogue-cannot-say-an-operation-is-public.md).
   Fixing the shape here would settle it before the catalogue can express it, which is the same
   mistake one layer up.

Recommendation: a follow-up story sequenced behind C-235, which would let `effects` be *declared*
and make `effects_derived` unnecessary rather than constant.

## Notes
- Consumer: flux-exchange `console/`, story
  [X-07](https://github.com/codewandler/flux-exchange/blob/main/docs/stories/X-07-console-reads-the-catalogue.md).
  It reported these rather than patching the components, which is the contract in that repo's
  `AGENTS.md` and the reason this story exists instead of a silent local fork.
- The components are the shared artifact. A fix here reaches both consumers; a fix in either console
  reaches one and drifts.
- Related: [C-235](C-235-the-catalogue-cannot-say-an-operation-is-public.md) is the same *shape* of
  bug one layer down — the catalogue emitting `[]` for both a withheld and a positively-public
  operation, so no host reading it can tell them apart. Worth designing the two together: a
  distinction the catalogue cannot express is one the components cannot render.
