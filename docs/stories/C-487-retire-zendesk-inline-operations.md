---
id: C-487
title: "Make every Zendesk operation spec-sourced"
pillar: Connector
status: ready
priority: 2
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [providers, openapi, zendesk]
note: "greenfield cleanup — replace seven Support and two Messaging inline transcriptions; no compatibility constraint"
---

# Make every Zendesk operation spec-sourced

## Goal

Remove the remaining nine hand-authored Zendesk operations so every Zendesk catalogue operation is
derived from vendored first-party API evidence.

## Acceptance

- [ ] A failing-first test enumerates the current nine `spec_source = null` operations and requires
      zero after the change.
- [ ] The seven Support operations select official counterparts despite `.json` path and response-
      requiredness differences; greenfield operation ids and contracts may change rather than being
      preserved through inline compatibility copies.
- [ ] The three prior write variants are represented honestly from `UpdateTicket` without duplicate-
      selector aliases or hand-authored wire surfaces.
- [ ] The Messaging response cycle is handled by a general bounded-schema policy with a regression
      test, allowing `PostMessage` and `ListMessages` to select from the official document.
- [ ] Zendesk's scoped build/diff, rehearsals, provenance checks and repository gates pass with all
      37 current operations spec-backed, or with a lower measured count only where collapsing the
      three duplicate writes is explicitly recorded as the greenfield source truth.

## Progress

- 2026-08-02: `jq` over `web/public/catalog.json` measured 37 operations: 28 spec-backed and nine
  inline. The nine are seven pre-existing Support operations plus Messaging message create/list.
- 2026-08-02: a read-only audit measured that none can migrate with today's overlay alone: Support
  differs in `.json` paths, required envelopes and one-to-three UpdateTicket mapping; Messaging is
  skipped on `message -> quotedMessage -> quotedMessageMessage -> message` recursion.

## Notes

- The owner explicitly states this is greenfield with no users and no contract to preserve. Do not
  keep an inline operation merely to retain its old id, path spelling, body flattening or response
  requiredness.
