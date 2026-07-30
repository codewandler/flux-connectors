---
id: C-126
title: "Raise response_schema coverage and put a floor under it"
pillar: Spec
status: ready
priority: 3
design: docs/designs/member-io-schemas.md
epic: member-io
areas: [providers, connector-spec]
note: "16 of 97 operations declare a response shape. The floor test matters more than the number — coverage that nothing watches only ever goes down"
---

# Raise response_schema coverage and put a floor under it

## Goal

Declare response shapes for the operations that lack them, and make the coverage figure a measured,
non-decreasing property rather than an accident.

## Acceptance

- [ ] **The floor test lands first, before any new schema.** A test reports current
      `response_schema` coverage and fails if it drops below the recorded floor. Measured today:
      **16 / 97**.
- [ ] Coverage rises meaningfully across the shipped providers, prioritising the operations a caller
      most needs to destructure — reads that return a single entity, and anything a flow branches on.
- [ ] Every schema added is **derived from the vendor's published documentation**, cited in the TOML
      the way `docs/designs/provider-operation-inventory.md` already cites wire shapes. A guessed
      schema is worse than none: it looks authoritative and is not.
- [ ] **Absence stays absence.** An operation whose response shape is genuinely unknown emits no
      schema — never `{}` and never a permissive `{"type": "object"}`, both of which pass a coverage
      count while telling a consumer nothing.
- [ ] The floor is raised to the new figure in the same commit, so the ratchet only turns one way.
- [ ] Generation stays **offline**: no operation's schema may be obtained by calling the vendor. That
      rule is absolute (`AGENTS.md`).
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **The floor is the deliverable; the number is the by-product.** Coverage nothing watches only ever
  decreases — a new connector ships without response shapes and the ratio quietly falls. The test is
  what makes the next hundred operations better.
- Do not chase 100%. Some vendor responses are genuinely unspecified or vary by account; recording
  that honestly is a better outcome than a schema nobody can rely on. Say in Progress which ones you
  deliberately left absent and why.
- This story declares what the **vendor sends**. It does not make that the operation's output type —
  that distinction is [C-127](C-127-truthful-output-typing.md), and conflating them is the failure
  mode the epic is built around.
- Beware error envelopes: several providers answer `200` with an error in the body (Slack's `ok`,
  Zendesk's flat-body silent ignore). A response schema that models only the success case is a
  half-truth; note the error shape where the inventory already documents it.
