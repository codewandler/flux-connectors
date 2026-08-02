---
id: C-474
title: "Integrate, prove and release the five-provider spec wave"
pillar: Agent
status: in-progress
priority: 15
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [providers, catalogue, release]
note: "GitHub, Stripe, Microsoft Graph, OpenAI and Twilio; coordinator-owned regeneration, gates and release"
---

# Integrate, prove and release the five-provider spec wave

## Goal

Combine the disjoint provider work into one internally consistent catalogue and ship it.

## Acceptance

- [x] Confirm each C-469 through C-473 scoped build/diff independently, then run one full-catalogue
      build over the integrated wave; regenerate rather than merge every whole-catalogue artifact.
- [x] The five operation counts each strictly exceed their measured baselines and every new operation
      is spec-backed, response-schema checked and request-rehearsed.
- [x] Response coverage floors/ceilings and declared catalogue counts are re-measured and updated only
      by the coordinator; C-81 is used rather than duplicating its count contract.
- [x] Full workspace build/test/clippy/fmt, catalogue diff, public-site build/tests and final diff are
      green.
- [ ] Changelogs describe user-visible coverage, C-468 through C-476 and C-481 close, and an ordinary
      release is cut. C-477 remains the explicit runtime-version compatibility follow-up because it
      would intentionally move established request bytes; no claim is made that this satisfies the
      separate new-provider release trigger because all five provider ids pre-existed.

## Progress

- 2026-08-02: integration started after all five provider-scoped build/diff and rehearsal gates were
  confirmed. The coordinator owns the remaining full-catalogue regeneration, complete gates, count
  ratchets, changelog polish, and release.
- 2026-08-02: integration gate complete. Counts moved GitHub 5→9, Stripe 8→12, Microsoft Graph
  8→12, OpenAI 4→8, and Twilio 5→9. The full plan measures 54 providers, 65 services, 735 operations,
  and 1005 artifacts; response coverage needed no ratchet change. Full Rust build/test/clippy/fmt,
  clean catalogue diff, public-site build plus 43 tests, and host-page 15 tests are green. Only the
  release checkbox remains.
