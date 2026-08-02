---
id: C-482
title: Render a legacy default service beside named siblings
pillar: Surfaces
status: done
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [web, catalogue]
note: "an explicit legacy default is a real filterable surface; only an implicit single-service default stays hidden"
---

# Render a legacy default service beside named siblings

## Goal

Keep the explorer's useful default-service elision for ordinary single-surface connectors without
hiding the already-published primary surface of a connector that now has named siblings.

## Acceptance

- [x] A provider whose only service is `default` still shows no service card, row label, or filter
      option.
- [x] A validated provider carrying `default` beside named services offers every operation's service
      in the facet, including the legacy default, and filtering it returns the primary operations.
- [x] The reserved token remains the machine value but is presented generically as `Primary`; no
      provider-specific label is hand-maintained in site source.
- [x] Provider cards and operation rows expose all multi-surface service memberships without moving
      published addresses.
- [x] The public-site build and all explorer tests pass against the regenerated catalogue.

## Progress

- 2026-08-02: filed from the full C-474 Node gate after its service-facet assertion proved Zendesk's
  21 Support operations belonged to a service the connector did not offer as an option.
- 2026-08-02: failing-first `npm run build && npm test` passed 40 of 43 tests and failed on the
  absent `default` facet, the missing Zendesk primary card/row, and the old service-card rule. A
  second focused failing-first run proved filtering itself was not independently testable before
  `operationMatchesView` was extracted from the component.
- 2026-08-02: `visibleServices` now omits only a sole implicit `default`; multi-surface defaults and
  explicitly named single services remain visible. `serviceLabel` maps the raw machine token
  `default` to generic `Primary` prose in the select, provider card, and operation row, while filter
  state and `data-service` attributes retain `default`. The extracted view matcher proves that
  selecting Zendesk/default returns exactly its 21 Support operations, and the catalogue-derived
  single-surface fixture proves the original omission remains.
- 2026-08-02: the complete public-site gate `cd web && npm ci && npm run build && npm test` passed:
  the locked install added 126 packages, VitePress built every page in 9.43 seconds, and all 43 Node
  tests passed. This Node tree declares no separate formatter or linter; `git diff --check` passed,
  VitePress compiled the TypeScript/Vue sources, and the suite's source-architecture lint assertions
  passed with the rest.
