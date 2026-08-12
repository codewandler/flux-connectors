---
id: C-548
title: "Source comments across five crates still cite C-10 as pending"
pillar: Codegen
status: ready
priority: 3
epic: catalog-artifact
areas: [connector-pack, connector-flux, connector-spec, catalog]
note: "C-545's implementor catalogued ~45 stale C-10 references across 34 files outside its write set — connector-pack/src/auth.rs:9 ('an intentional gap since C-10') is the loudest; all present superseded work as pending. Comment-only reconciliation, spanning crates one wave cannot hold disjointly"
---

# Source comments across five crates still cite C-10 as pending

## Goal

No source or test comment presents C-10 as pending work. C-535 closed C-10 as
superseded-never-implemented; C-542 fixed the emitted manifest header, C-545 the status route and
connector-cli's comments. What remains is the catalogued tail: ~45 references across 34 files in
`connector-pack/src` (auth.rs:9, lib.rs:332, rehearsal.rs:30, tool.rs:510), `connector-flux/src`
(lib.rs:45, op.rs ×6, types.rs:34), `connector-spec/src` (auth.rs:39, ir.rs ×2, provider.rs ×2),
`catalog/src/lib.rs:234`, and ~25 assertion messages and comments under `tests/` (line numbers
measured 2026-08-12 by C-545's implementor; re-verify before editing).

## Acceptance

- [ ] `grep -rn 'C-10' crates/` returns only honest history ("was C-10's", "closed as superseded")
      or documented negative tests — nothing that reads as pending work. Quote the final grep.
- [ ] Comment-only: no behaviour change, no emitted-artifact byte moves (full build writes
      nothing; `diff` clean), no test assertion weakened — an assertion *message* may be reworded,
      the asserted property may not.
- [ ] The successor named is C-534's program, matching C-542's header and C-545's status pointer.

## Progress

- 2026-08-12: Filed at C-545's integration from its implementor's adjacent findings.

## Notes

- Write set spans `connector-pack`, `connector-flux`, `connector-spec`, `catalog` source and test
  comments — collides with C-538 (connector-pack) and C-540 (connector-flux); schedule after
  C-538 lands, and skip any file C-540 is about to delete.
