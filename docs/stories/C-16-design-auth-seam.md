---
id: C-16
title: Design the $auth seam and file its stories on flux's board
pillar: Bridge
status: ready
priority: 2
design: docs/designs/auth-seam.md
epic: connectors-v1
areas: [flux-bridge]
note: **critical path** · ships in ../flux, longest lead time
---

# Design the $auth seam and file its stories on flux's board

## Goal
Settle the design for scheme-aware credential injection in flux's `http.request`, and get the
implementation stories onto `../flux`'s board early — it ships on a different repo's release cadence
and blocks milestone 1's finish.

## Acceptance
- [ ] [docs/designs/auth-seam.md](../designs/auth-seam.md) reviewed and its open question resolved
      **with flux**: does flux want a separate connector-manifest registry, or should connector auth
      fold into the existing plugin manifest registry?
- [ ] Implementation stories filed on `../flux`'s board covering: the `{"$auth": {...}}` header
      marker, `AuthScheme` reuse from `flux-plugin-protocol`, deny-by-default purpose resolution,
      redactor registration of the composed value, `http_hosts` scoping, and `Query`-scheme injection
      as its own story.
- [ ] Each filed story names its failing-first test.
- [ ] This repo records which flux release the seam is expected in, so `C-15` knows what to wait for.

## Progress
- (not started)

## Notes
- Do **not** implement the flux change from this repo; file the stories and let flux's own workflow
  run. This story is done when the design is settled and the work is queued there.
- The rejected fallback (operator stores a pre-composed `Authorization` value) is documented in the
  design as an emergency option only.
- Key flux files: `crates/flux-web/src/http.rs:234` (`resolve_header_value`),
  `crates/flux-plugin-protocol/src/lib.rs:344` (`AuthScheme`).
