---
id: C-539
title: "Sibling adoption: Exchange reads the artifact"
pillar: Bridge
status: backlog
priority: 2
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [release, docs]
note: "Cut the schema release Exchange consumes; zero runtime Flux parses remain in exchange-host; .connector.toml stays emitted as a projection until flux/D-214 repoints inbound"
---

# Sibling adoption: Exchange reads the artifact

## Goal

Land the release and coordination work that lets `flux-exchange` consume the reader and resolver —
ending its runtime Flux parsing — without breaking either sibling, per the Decision 0022 migration
rule and the Decision 0008 schema-release contract.

## Acceptance

- [ ] A `codewandler-connector-*` release carries the pack, reader and document-backed resolver;
      its notes name the schema version and the exact consumer action, and the four-crate publish
      closure still derives (`scripts/publish-crates-io.sh --print-order`).
- [ ] Adoption stories are filed in `../flux-exchange` covering: swapping the catalogue routes to
      the reader, replacing the four `Rehearsal` call sites in
      `crates/exchange-host/src/settings.rs` (416, 471, 1457, 3449 — re-measure before filing),
      and preserving the grant-gated `Granted::resolve`/`Admitted::resolve` topology unchanged.
- [ ] After Exchange's adoption release: `grep -rn 'Rehearsal' ../flux-exchange/crates` reports
      zero production call sites, and Exchange's own gate proves invoke behaviour unchanged
      against its existing fixtures.
- [ ] `connectors/<name>.connector.toml` is emitted from the canonical document as a projection,
      byte-compatible for the fields `flux-channels`' connector arm reads, with a test pinning
      that compatibility; its retirement is explicitly owned by flux/D-214, not this story.
- [ ] `docs/integrating-with-flux.md` is re-measured and updated to describe Paths A/B over the
      artifact, and the flux-side statement remains true: nothing at the invoke seam changed.

## Progress

- (not started)

## Notes

- Blocked on C-537 and C-538. This story owns the connectors-side obligations and the
  coordination; Exchange-side implementation lives in Exchange's tracker.
- Do not delete anything here — deletion is C-540's, gated on this story's adoption evidence.
