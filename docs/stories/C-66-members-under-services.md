---
id: C-66
title: Put inbound events under a service, and admit AsyncAPI as their front-end
pillar: Spec
status: ready
priority: 5
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-spec, connector-cli]
note: provider → service → (operation | event) · the two gaps C-58's epic leaves open
---

# Put inbound events under a service, and admit AsyncAPI as their front-end

## Goal
Close the two seams between the [inbound-events epic](C-58-inbound-events-epic.md) and the rest of the
model: an event belongs to a **service**, not to a provider, and a vendor's **AsyncAPI** document should
be able to supply events the way OpenAPI supplies operations.

## Acceptance
- [ ] **`[inbound]` moves under the service.** C-59's section and C-49's `Service` level meet here:
      AWS's `s3` and `bedrock-runtime` version and host independently, and their events do too, so a
      provider-level `[inbound]` cannot describe a multi-service vendor. A provider that declares no
      service keeps working unchanged through the `default` service.
- [ ] **Operations and events share one name namespace per service.** A collision is a loud error, as
      C-3 already treats duplicate op ids — both project into the same address space and into flux's
      declaration namespace, so the collision has to be impossible rather than merely unlikely.
- [ ] **An event is addressable** under its service's gid, round-tripping like every other address
      (C-37, C-49). Decide and record whether it reuses `#name` — which the shared namespace above
      implies — or takes its own separator.
- [ ] **Selecting a service selects its events too.** `--service <name>` yields that service's
      operations *and* its events; a test proves a service selection is not silently operations-only.
      This is the "enable the whole service" act, and an event surface that silently stayed behind
      would be the worst kind of partial success.
- [ ] **AsyncAPI 3 is specified as a front-end**, mirroring C-4's shape for OpenAPI: ingest takes
      **bytes** so `connector-spec` stays hermetic and unit-testable. The design records the mapping —
      `channels` and their `address`, `messages` and their payload schemas, `operations` with
      `send`/`receive` giving direction, `servers`, and the protocol `bindings` that name the transport
      — and which parts of C-59's `[inbound]` each one supplies.
- [ ] **What AsyncAPI cannot supply is named explicitly**, because that is the part that decides
      whether ingest is worth writing: signature verification parameters (C-60's matrix), the
      discriminator, the delivery id, and tolerance windows are not in the document. State whether a
      vendor's AsyncAPI file therefore yields a *complete* inbound declaration or only its skeleton
      plus a required patch layer.
- [ ] Implementation of the ingest itself may be deferred to its own story — but the mapping and the
      hermetic-boundary decision land here, so that a hand-authored `[inbound]` written today is
      already the shape ingest would produce.

## Progress
- Not started. Filed 2026-07-30 from a user request that events be treated like operations — inline or
  from AsyncAPI — and enabled with their service.

## Notes
- **This story exists because two efforts met.** The inbound-events epic (C-58–C-65) was filed
  concurrently with the service level (C-49) and does not reference it; without this, `[inbound]` lands
  at provider level and has to move again as soon as a multi-service vendor arrives.
- **Sequence after [C-49](C-49-provider-services.md) and [C-59](C-59-inbound-ir-and-toml.md).** It is
  cheaper as an amendment to C-59's section than as a migration after it ships — the same argument
  C-49 makes against letting C-37 publish an address scheme first.
- **Outbound streams are a separate question this does not settle.** A WebSocket or SSE subscription
  (Slack Socket Mode, flagged as an open question in `docs/designs/inbound-events.md`) is inbound in
  data direction but "we connect and hold a stream" in operation, and C-63's poll transport already
  establishes that inbound is an abstraction over transports. Whether Socket Mode is a third transport
  under that abstraction belongs with C-63 or C-64, not here.
