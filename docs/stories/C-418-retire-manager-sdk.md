---
id: C-418
title: "Retire manager-sdk — the caller's migration and the three gaps that block it"
pillar: Bridge
status: backlog
priority: 3
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [bridge, providers]
note: "owner-decided 2026-08-01: callers reach babelforce through connectors-api or flux ops, and this repo grows no language emitters. The claim does not tick until multipart, form bodies and the 23 nameless operations are resolved or scoped out"
---

# Retire manager-sdk — the caller's migration and the three gaps that block it

## Goal
State what a `manager-sdk` caller does instead, prove it on a real call, and close or explicitly scope
out the three gaps that stand between full coverage and an archivable SDK.

## Acceptance
- [ ] A migration note in `docs/` maps the SDK's surface onto this one: its 36 resource namespaces
      onto services, its auto-pagination onto `quirks.pagination`, its typed errors onto the error
      envelope, and its one-shot `ManagerClient.connect` onto the credential model.
- [ ] **One real call, end to end**, through `connectors-api` against a live babelforce account, with
      the request and response recorded the way `crates/connectors-api/README.md` records the first
      one.
- [ ] **Gap 1 — multipart.** `BodyEncoding` is `Json | Form`, so five upload operations cannot be
      emitted. Either a variant lands or the five are recorded as out of scope with what a caller does
      instead.
- [ ] **Gap 2 — form bodies.** The three `application/x-www-form-urlencoded` operations are the OAuth
      token endpoints, and `docs/roadmap.md:26` records the encoder as upstream flux work (`L-101`).
      So **the login flow is what is blocked.** Resolve against C-135/C-136 — a credential-producing
      operation returns a handle, never the token — or record the dependency with its upstream story.
- [ ] **Gap 3 — descriptions.** No exposed operation reaches a model without a description (C-417).
- [ ] The retirement is stated somewhere a manager-sdk reader will find it, and this repo's
      `docs/roadmap.md` records the change of ownership.

## Progress
- (not started)

## Notes
- The owner chose *retire it* over *absorb it* (emit typed clients here) and *feed it* (publish the IR
  and keep the SDK) on 2026-08-01; both alternatives and their reasons are in the design.
- **This story owns the honesty of the claim.** Partial coverage that reads as full is worse than
  documented partial coverage, because a caller discovers the gap at runtime.
- Nothing here archives the other repository — that is a decision for its owners, taken once this side
  is demonstrably ready.
