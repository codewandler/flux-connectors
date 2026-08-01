---
id: C-408
title: "The explorer components cannot say 'this source does not publish that', so a thinner catalogue reads as a claim about the connector"
pillar: Surfaces
status: ready
priority: 3
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

- [ ] A component can distinguish **absent** from **not published by this source**, and renders them
      differently. The mechanism is this story's design decision — an `Auth | null` where `null`
      means unpublished, a source-capability descriptor, or something better — but a bare empty
      collection must stop carrying two meanings.
- [ ] **Failing-first test** — a catalogue document that omits `auth` does **not** render "not
      configured", and does not render in the danger colour. This is the regression that motivated
      the story.
- [ ] Same for `credentials`: an unpublished credential set does not render as "live calls are
      disabled".
- [ ] `method`/`path` are omitted rather than rendered as empty chips when unpublished.
- [ ] The 15 components still import only Vue, a sibling, and `catalog.mts` — the invariant
      `no_component_imports_the_site_framework` asserts. Whatever carries the distinction arrives as
      a prop or as injected context; **no component learns which source it is rendering.**
- [ ] `web/`'s own rendering of the full catalogue is unchanged — this is additive, and a
      byte-identical render of `public/catalog.json` is the proof.

## Also, from the same consumer

- [ ] **`Operation` in `catalog.mts` has no `effects`, `effects_derived` or `admitted`.** That is the
      field set flux-exchange's catalogue route exists to publish — `risk` and `idempotency` are
      there, effects are not — so a host serving grant-relevant metadata has to carry it alongside
      the shared type rather than in it. Decide whether the shared `Operation` gains them. Note that
      `effects` there is *derived* (from non-empty `hosts`) rather than declared, so a flag saying so
      travels with it or the field lies.

## Progress
- (not started)

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
