---
id: C-20
title: Emit auth from the unified model
pillar: Codegen
status: backlog
design: docs/designs/unified-auth.md
epic: unified-auth
areas: [connector-flux]
---

# Emit auth from the unified model

## Goal
Make codegen consume the unified model: emit one credential reference per requirement and
declare the full method in the manifest, so the generated Flux stays free of assembly logic.

## Acceptance
- [ ] Generated Flux **names credentials only** — no prefix strings, no base64, no token assembly
      anywhere in a `.flux` file.
- [ ] An AND requirement set emits one marker per credential, so a request can carry two of them in
      two different placements.
- [ ] Alternative requirement sets resolve by the documented rule (first set whose credentials are all
      configured), and the selected alternative is **recorded in the manifest** so regeneration is
      stable and reviewable.
- [ ] An operation with an explicit empty requirement set emits no credential at all.
- [ ] The manifest serializes `source`/`acquire`/`place` per method, and round-trips.
- [ ] A test asserts no generated artifact contains a credential value.

## Progress
- (not started)

## Notes
- Depends on C-19 (the model) and C-10 (marker + manifest emission). C-10 and this story overlap
  heavily — if C-10 has not started when this is picked up, fold C-10 into it rather than doing the
  work twice.
- Query and cookie placements affect the URL and the request, not just headers — the emitter needs a
  path for each, which is also what flux's seam must grow (see [auth-seam](auth-seam.md)).
