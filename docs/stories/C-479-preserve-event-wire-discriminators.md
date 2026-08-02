---
id: C-479
title: Preserve vendor event discriminator values separately from local names
pillar: Surfaces
status: ready
priority: 4
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [connector-spec, connector-flux, connector-catalog]
note: "Zendesk values such as zen:event-type:ticket.created are not legal member names and must not be normalized"
---

# Preserve vendor event discriminator values separately from local names

## Goal

Let an inbound event use a safe local Flux member name while matching the vendor's exact wire
discriminator value, without weakening member-address validation or silently renaming vendor data.

## Acceptance

- [ ] `EventDecl` represents local member identity and exact wire discriminator value as distinct,
      validated concepts, with a backward-compatible default for existing providers.
- [ ] Lowering, generated metadata, catalogue publication, and host dispatch preserve and use the exact
      wire value while public member addresses retain the safe local name.
- [ ] Mutation tests cover `:` and `/` in wire values, reject empty/ambiguous values, and prove existing
      event providers remain byte-stable unless they opt into the new field.
- [ ] Zendesk event values such as `zen:event-type:ticket.created` can be represented losslessly.

## Progress

- 2026-08-02: filed from C-465's fail-closed webhook implementation review.
