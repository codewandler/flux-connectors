---
id: C-460
title: "Curate the Zendesk suite operation and inbound inventory"
pillar: Spec
status: done
priority: 1
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [providers, specs, docs]
note: "decide carry/withhold/defer from the pinned bytes before widening selection; do not confuse an OAS path list with a connector"
---

# Curate the Zendesk suite operation and inbound inventory

## Goal
Turn the vendor documents and official webhook prose into a reviewed Zendesk suite boundary before
provider TOML starts selecting endpoints.

## Acceptance
- [x] [The inventory](../designs/zendesk-suite-inventory.md) gives every candidate in Support
      foundations, sync/custom data, Help Center, Messaging, and Webhooks a carry/withhold/defer
      decision and reason.
- [x] It records service, host, version, authentication/config, response-shape gaps,
      pagination/query shape, request media type, safety/idempotency, and verification requirements.
- [x] Multipart-only, credential/session/password/OAuth-secret responses, unencodable query/path
      inputs, and missing OAS request bodies are reported rather than silently dropped.
- [x] Facts from the three current first-party OAS documents are separated from facts sourced from
      official Webhooks prose, with the commands and source URLs that produced every count.
- [x] Talk, AI Agents, Sell, and legacy Chat receive an explicit out-of-epic decision.
- [x] C-461 through C-465 each have an exact first-tranche set; later implementors remeasure against
      C-459's pinned bytes instead of copying a mutable-URL count into provider TOML.

## Progress

- 2026-08-02 — Inventory completed against the live official Ticketing, Help Center, and Sunshine
  Conversations documents plus the official Webhooks reference. The source hashes and the commands
  that produced them are recorded in the design so C-459 can distinguish what was reviewed from
  what it ultimately vendors.
- The most important finding is not an endpoint count: Zendesk's HMAC is already an exact row in
  `HmacSpec`, but official event discriminator values contain `:` and the member-address grammar
  refuses `:` while `EventDecl` promises to preserve vendor names. C-465 may ship administration
  operations independently, but inbound events stay deferred until that conflict is resolved
  explicitly.
- C-6 and C-14 remain linked owners: C-6 adds request parameters omitted by a vendor document; C-14
  owns fetch and upstream drift. C-6 does not own response-schema replacement and its current
  implementation refuses an unmatched parameter, so operations with an incomplete whole contract
  stay deferred rather than being assigned to a hypothetical patch. This story duplicates neither
  capability.
- 2026-08-02: implementation preflight corrected Messaging's webhook family: four pinned response
  schemas return the live signing secret, so C-430 withholds them and the otherwise response-safe
  delete stays deferred rather than becoming an orphaned destructive operation. C-464's exact
  first tranche is nine operations.
- 2026-08-02: Support preflight also corrected `CreateOrUpdateUser` from carry to withhold. Its
  `UserInput` union exposes nested `password` in both variants and permits a merge body with no stable
  identity; the current overlay can do neither a nested omission nor a stronger union. C-466 now
  carries eight reads and no writes.

## Notes
- This story changes documentation only and can run beside C-458 and C-459.
- Implementation boundary: [zendesk-suite-inventory.md](../designs/zendesk-suite-inventory.md).
