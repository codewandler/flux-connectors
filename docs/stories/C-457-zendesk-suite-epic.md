---
id: C-457
title: "Zendesk suite — vendor specs, stable Support addresses, and curated surfaces (epic)"
pillar: Spec
status: done
priority: 1
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [connector-spec, connector-cli, connector-flux, providers, specs]
note: "EPIC — preserve the seven published Support addresses, make first-party specs the source, and add curated Support, Help Center, Messaging, and webhook surfaces"
---

# Zendesk suite — vendor specs, stable Support addresses, and curated surfaces (epic)

## Goal
Expand the shipped Zendesk connector from seven hand-authored Support calls into a curated,
spec-backed suite without moving an existing operation or publishing unsafe vendor surface.

## Acceptance
- [x] A published default service can coexist explicitly with named services without changing its
      addresses or artifacts ([C-458](C-458-preserve-default-service-while-growing.md)).
- [x] Zendesk's Ticketing, Help Center, and Messaging documents are pinned, scrubbed, and provenanced;
      builds remain offline ([C-459](C-459-vendor-zendesk-openapi-documents.md)).
- [x] Every candidate operation and inbound surface has a carry/withhold/defer decision grounded in
      the pinned bytes or identified official prose ([C-460](C-460-curate-zendesk-suite-inventory.md)).
- [x] C-6's parity preflight measures the seven shipped Support operations against the full Ticketing
      document and keeps them inline after proving the current overlay cannot preserve their paths,
      response contracts, repeated write variants, or conditional-replay evidence.
- [x] Query-free ticket audit history proves the first Zendesk spec selection, followed by the
      inventory-approved Support foundations and synchronization/custom-data slices
      ([C-461](C-461-expand-zendesk-support-foundations.md),
      [C-466](C-466-expand-zendesk-support-foundations.md),
      [C-462](C-462-add-zendesk-sync-and-custom-data.md)).
- [x] Help Center and Messaging land as named services with service-specific contracts
      ([C-463](C-463-add-zendesk-help-center.md), [C-464](C-464-add-zendesk-messaging.md)).
- [x] Webhook administration and any inbound surface are either verified exactly or withheld with a
      named model gap ([C-465](C-465-add-zendesk-webhooks.md)).
- [x] Multipart-only and credential-returning operations remain withheld; query-unsafe operations do
      not become callable by accident.
- [x] A coordinator full build regenerates the whole-catalogue artifacts after the serial provider
      stories integrate, and the workspace gate is green.

## Progress
- 2026-08-02 — Epic filed after measuring the shipped connector and the first-party documents. The
  first safe new operation is `ListAuditsForTicket`, selected without its optional query parameters.
- 2026-08-02 — First wave integrated: the address-preserving legacy-default service model, all three
  pinned documents, the suite inventory, and query-free ticket audit history are complete. Full
  regeneration reports `952 artifacts up to date (54 providers checked)` and the workspace test gate
  is green. C-6 and the serial surface stories remain.
- 2026-08-02: the C-6 parity premise was disproved rather than forced. Same-session OAS inspection
  found matching methods but `.json`-less paths and five response envelopes with empty `required`
  sets; the overlay hard-wires spec path/response and cannot select `UpdateTicket` three times. The
  seven stable inline blocks total 225 declarative lines including their table headers and remain the
  smaller honest representation until the overlay grows several independent capabilities.
- 2026-08-02: integration complete. Zendesk measures 37 operations: 21 on the address-preserving
  Support service, seven on Help Center, and nine on Messaging. A full build and diff report `1005
  artifacts up to date (54 providers checked)`; the workspace gate, 43-test public-site gate, and
  15-test host-page gate are green. Webhook CRUD and inbound delivery remain deliberately withheld
  behind C-479 and C-480 instead of shipping an unsafe partial lifecycle.

## Notes
- C-14 owns generic fetching and upstream drift checks; this epic vendors bytes and records provenance
  without duplicating that machinery.
- Provider stories are serial: they share `providers/zendesk.toml` and Zendesk generated artifacts.
