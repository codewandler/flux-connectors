---
id: C-36
title: Prove the proxy and the Flux emitter build the same request
pillar: Bridge
status: blocked
design: docs/designs/connectors-proxy.md
epic: connectors-proxy
areas: [connector-flux, flux-bridge]
note: blocked on C-34 · two backends over one IR will drift without this
---

# Prove the proxy and the Flux emitter build the same request

## Goal
Keep the two consumers of the IR honest with each other, so the documented curl and the executed Flux
never describe different requests.

## Acceptance
- [ ] A conformance test drives both the Flux emitter and the proxy's request builder from the same
      IR operation and asserts they agree on method, URL, query, headers and body.
- [ ] The test covers every operation in every shipped provider, not a sample.
- [ ] A divergence fails CI, naming the operation and the field that differs.
- [ ] Ideally the two share one request-building function rather than agreeing by test — the test is
      the floor, not the goal.

## Progress
- **Blocked on C-34.**

## Notes
- Two independent backends over one IR is exactly the shape that drifts silently: each is
  individually plausible, and nothing compares them. A generated curl that no longer matches the
  operation it documents is worse than no curl at all, because it looks authoritative.
- Related: C-29 (body modelling) and C-30 (query encoding) apply to both backends. The proxy inherits
  those gaps rather than escaping them.
