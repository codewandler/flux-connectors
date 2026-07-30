---
id: C-44
title: Build the provider and operation explorer
pillar: Surfaces
status: backlog
design: docs/designs/public-docs.md
epic: public-docs
areas: [web]
note: the reason the site exists · needs C-42 and C-43
---

# Build the provider and operation explorer

## Goal
Let someone browse every provider and operation, filter to what they need, and deep-link to one —
turning a repo of generated artifacts into something evaluable in a minute.

## Acceptance
- [ ] Provider list: vendor, operation count, auth scheme, and a status badge that does not flatter.
- [ ] Operation list, filterable by provider, risk, idempotency, and **whether it currently works**.
- [ ] Operation detail: signature and typed parameters from the JSON Schema, the **generated Flux**
      verbatim, and the credentials and hosts it needs.
- [ ] **Deep links per operation**, so the site is referenceable from an issue or a chat.
- [ ] An operation that does not work says so prominently, wherever it appears.
- [ ] All data read from the generated `catalog.json` (C-42). **Nothing hand-maintained.**
- [ ] Works without JavaScript for at least the operation content, or degrades to something useful.

## Progress
- **Blocked on C-42** (the data) and **C-43** (the site).

## Notes
- Modelled on the pattern in `~/babelforce/projects/ai-agent-platform/web/packages/console` — list →
  detail, `CapabilityPicker.vue`, `CommandPalette.vue` — but **read-only and static**. That console
  talks to a live API; this has no backend and must not grow one (`vision.md` lists a runtime as a
  non-goal).
- Plain Vue components over a JSON file. There is no state-management problem here worth Pinia.
- Once C-37 lands, deep links should use the `oip` address rather than the local symbol.
