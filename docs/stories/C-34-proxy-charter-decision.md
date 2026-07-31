---
id: C-34
title: Decide whether a connectors proxy belongs in this project
pillar: Bridge
status: done
priority: 7
design: docs/designs/connectors-proxy.md
epic: connectors-proxy
areas: [flux-bridge]
note: **decision, not code** · blocks the whole epic · contradicts a stated non-goal
---

# Decide whether a connectors proxy belongs in this project

## Goal
Settle, deliberately and in writing, whether flux-connectors ships a credential-injecting proxy —
because doing so reverses a non-goal the vision states explicitly, and that reversal should be a
decision rather than a drift.

## Acceptance
- [x] The conflict is resolved one way or the other, and the reasoning is recorded.
      `docs/vision.md` currently says: *"**A runtime.** This repo compiles; flux executes.
      flux-connectors ships no server, no daemon, and no request path of its own."* A proxy is all
      three.
- [x] If **yes**: `vision.md`'s non-goal is amended to say what changed and why, and the epic's
      remaining stories are promoted.
- [x] If **no**: this epic is closed as out of scope, and the alternative that keeps the benefit is
      named — most likely a **separate repository**, since the connector manifest is a public
      artifact and a proxy consuming it needs no privileged access to this codebase.
- [x] Either way, the relationship to the [`$auth` seam](../designs/auth-seam.md) is stated. The two
      solve the same problem by different means; shipping both without saying which is primary is the
      outcome to avoid.

## Progress

- **2026-07-31 — DECIDED: yes, narrowed.** The conflict is resolved in favour of a reference host,
  not the credential-injecting proxy this story was filed about.

  **What was permitted.** `docs/vision.md`'s non-goal became *"A runtime **for production traffic**"*
  — this repo may ship `crates/connectors-app`, a loopback-bound reference host that proves the seams
  end to end (OAuth callback, credential store, operation execution), never published and never a
  production request path. Designed in [connectors-app.md](../designs/connectors-app.md).

  **What was refused, and it is the thing the story was actually about.** A credential-injecting
  proxy reachable by other callers is still out. This story's own Notes name why and the reasoning
  stands unamended: *"a credential-injecting proxy's entire job is to add authority its caller does
  not have. Unauthenticated and reachable, it is a credential-lending service for whoever finds it."*
  A loopback host that holds a credential for its own single operator is a categorically smaller
  claim than a service that lends authority to callers.

  **The `$auth` seam relationship, which the acceptance required be stated.** The two no longer
  compete. C-114/C-115/C-116 landed `connector-pack`, which assembles auth in Rust and delegates to
  flux's own `http.request` — so the execution path does not wait on a flux release, and the seam is
  narrowed to the composite case rather than being the primary route. The proxy's original
  justification ("every provider is blocked on the `$auth` seam landing in another repo on someone
  else's schedule") no longer holds, which is part of why the narrow answer is the right one.

  **The risk this story named is accepted deliberately, not dissolved.** It is still true that a host
  here is the first component holding plaintext credentials at runtime. The mitigations are that it
  is loopback-only, unpublished, and outside the compiler crates — and that both existing fences
  (`no_network.rs`, `dependency_fence.rs`) are extended to name it explicitly, so the exception is
  visible rather than accidental.

## Notes
- **What makes it tempting.** Every provider is currently blocked on the `$auth` seam landing in
  *another repo* on someone else's schedule. A proxy is an execution path that does not depend on
  that at all, and it makes the generated curl examples secret-free.
- **What makes it serious.** It would be the first component here that holds plaintext credentials at
  runtime — every other artifact in this repo is inert text. That is a categorical change in what a
  vulnerability costs, not an incremental one.
- **The confused-deputy problem is inherent, not incidental.** A credential-injecting proxy's entire
  job is to add authority its caller does not have. Unauthenticated and reachable, it is a
  credential-lending service for whoever finds it. flux reached the same conclusion about its own
  HTTP server and refuses a non-loopback bind without a token.
- **The quiet risk:** a proxy could become the primary execution path, making flux optional and
  inverting this project's relationship to it. That might be a good outcome — it should not be an
  accidental one.
