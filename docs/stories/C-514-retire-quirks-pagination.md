---
id: C-514
title: "Retire quirks.pagination into the datasource binding's cursor vocabulary"
pillar: Spec
status: backlog
design: docs/designs/vendor-datasource-declarations.md
epic: vendor-datasources
areas: [connector-spec, providers]
note: "re-measured 2026-08-04: pagination still has no reader outside the loader, and two providers declare it (twilio ×2, babelforce patches ×2 — zendesk's 2026-07-31 row is stale). Decision 0006 rule 6: superseded by the [[datasources]] cursor vocabulary and REMOVED, not left as another declared-but-unreachable surface"
---

# Retire `quirks.pagination` into the datasource binding's cursor vocabulary

## Goal

Remove the dead `quirks.pagination` surface now that the `[[datasources]]` binding owns the cursor
and paging vocabulary — so how a vendor pages is stated once, where a consumer can reach it, instead
of twice with one copy unreachable.

## Acceptance

- [ ] `Quirks::pagination` and the `Pagination` enum (`crates/connector-spec/src/ir.rs:549` and
      `:486` as of 2026-08-04) are removed from the IR, the loader, the provider-TOML schema and
      the `HashDomain` destructuring. `deny_unknown_fields` then makes a leftover declaration a
      loud load error, not a silent drop.
- [ ] The existing declarations are dispositioned, each in the same diff: twilio
      (`providers/twilio.toml:306`, `:385`) and babelforce (`providers/babelforce.toml:600`,
      `:714`, both patches) either migrate into a datasource binding's cursor mapping or are
      removed with the reason reviewed in this story — an explicit reviewed removal, never a quiet
      deletion.
- [ ] [connector-surfaces.md](../designs/connector-surfaces.md)'s table and dead-surface rows are
      updated; `quirks.pagination` leaves the "reaches no artifact" set by ceasing to exist.
- [ ] The `ir_sha256` / `connectors.lock` movement is recorded as an intentional versioned schema
      change, per C-497's precedent for schema changes that alter no artifact bytes for most
      providers.
- [ ] **Failing-first test:** a provider TOML declaring `[operations.quirks.pagination]` is refused
      by the loader with a message pointing at the datasource binding's cursor vocabulary.
- [ ] The gate is green; the build stays a fixed point.

## Progress

- (not started)

## Notes

- Authority: Decision 0006 rule 6 — *"The dead `quirks.pagination` surface is superseded by the
  member's cursor vocabulary and removed rather than left as another declared-but-unreachable
  surface."* Design: [vendor-datasource-declarations.md](../designs/vendor-datasource-declarations.md).
- Depends on [C-512](C-512-datasources-ir-member.md) supplying the vocabulary that supersedes it —
  removal before replacement would delete the only place the twilio/babelforce paging facts are
  stated.
- `quirks.rate_limit` is deliberately out of scope: it has no producers *and* no consumers and its
  probable deletion is connector-surfaces.md's separate finding, not this supersession.
