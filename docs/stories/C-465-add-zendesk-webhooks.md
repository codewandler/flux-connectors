---
id: C-465
title: "Withhold Zendesk webhook administration and inbound events safely"
pillar: Surfaces
status: done
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [providers, connector-spec, connector-flux]
note: "official webhook prose, not the Ticketing OAS, is the source; signature verification must be exact or inbound stays withheld"
---

# Withhold Zendesk webhook administration and inbound events safely

## Goal
Account for Zendesk webhook administration and verified inbound events without turning a
credential-bearing response, incomplete setup lifecycle, or normalized discriminator into a
connector capability.

## Acceptance
- [x] Official prose is pinned as the source of the webhook decision because the Ticketing OAS has no
      `/api/v2/webhooks` paths and no first-party webhook OAS was found.
- [x] The five CRUD requests are inventoried exactly, but the complete family remains withheld while
      the generic response representation may expose `signing_secret`; response-schema omission is
      explicitly not treated as redaction, and update/delete do not ship as orphaned lifecycle calls.
- [x] Destination-auth inputs and dedicated signing-secret endpoints are named as credential hazards,
      never exposed as ordinary model-visible fields.
- [x] Zendesk's exact HMAC scheme is recorded as representable, while events/channels remain withheld
      behind focused model-gap stories for wire discriminator fidelity and subscription/secret setup.
- [x] Focused negative tests and the provider-scoped gate prove the withheld surface does not leak into
      provider declarations or generated metadata.

## Progress

- 2026-08-02: implementation preflight found that the generic official Webhook representation has an
  optional `signing_secret`. Because operation responses are returned raw, list/show/create cannot be
  made safe by narrowing response metadata; update/delete alone would be orphaned lifecycle calls.
  The outbound family therefore stays withheld rather than adding a `webhooks` service.
- 2026-08-02: the exact HMAC is already representable, but lossless event discriminator values and
  create/select/store subscription provisioning require C-479 and C-480 respectively.
- 2026-08-02: `zendesk_webhooks` now accounts for five ordinary CRUD and two credential-returning
  endpoints, proves the pinned Ticketing OAS has no webhook path, and fences the provider IR and
  generated Rust metadata against a Webhooks service, event, channel, or endpoint. The focused run
  passed 3/3 tests; `vendored_zendesk_specs` passed 5/5; clippy and formatting were clean; and
  `cargo run -p connector-cli -- diff --provider zendesk` reported
  `36 artifacts up to date (1 provider checked)`.

## Notes
- A webhook endpoint is an inbound declaration, not a long-polling operation.
