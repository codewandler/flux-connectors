---
id: C-484
title: "Vendor and normalize the official Asterisk ARI descriptions"
pillar: Spec
status: in-progress
priority: 1
design: docs/designs/asterisk-ari-rest.md
epic: asterisk-ari-rest
areas: [providers, openapi, asterisk]
note: "first-party Swagger 1.1/1.2 becomes deterministic OpenAPI 3 input; exact REST/WebSocket accounting"
---

# Vendor and normalize the official Asterisk ARI descriptions

## Goal

Make Asterisk's first-party ARI descriptions hermetic input to the existing OpenAPI front-end without
hand-transcribing their operations.

## Acceptance

- [ ] A failing-first vendored-spec test names the absent Asterisk source contract.
- [ ] The exact 11 raw documents, licence, upstream tag/commit and hashes are committed under
      `specs/asterisk/` with a reproducible public-source vendor script.
- [ ] A deterministic normalizer produces OpenAPI 3 from those bytes and refuses unknown source
      versions, document inventories, types, placements, duplicate operation ids, or unaccounted
      operations.
- [ ] Tests measure exactly 109 source operations, emit exactly 108 REST operations, and identify
      `events.eventWebsocket` as the sole deferred operation.
- [ ] Normalization preserves paths, methods, parameters, response models, descriptions and source
      identity rather than becoming a second hand-authored contract.

## Progress

- 2026-08-02: raw hashes and the 11/76/109/1/108 census were re-measured from the already pinned
  first-party Asterisk 22.10.1 bytes; implementation has not yet copied them into this repository.

## Notes

- C-485 consumes the normalized document. This story does not add a provider or generated catalogue
  output.
