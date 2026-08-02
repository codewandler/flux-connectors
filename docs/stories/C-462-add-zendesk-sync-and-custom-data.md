---
id: C-462
title: "Add Zendesk synchronization and custom data"
pillar: Spec
status: done
priority: 2
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [providers, connector-flux]
note: "custom objects and incremental exports only where cursor, query, and caller-chosen path values are encoded safely"
---

# Add Zendesk synchronization and custom data

## Goal
Add the approved custom-object and incremental-export calls as an honest data/sync surface.

## Acceptance
- [x] The selected operations exactly match C-460's approved custom-data and sync rows.
- [x] Cursor and pagination contracts are represented from the document and exercised in request
      composition; free-form search/filter inputs remain withheld while encoding is unsafe.
- [x] Caller-chosen custom-object keys land only if path encoding or a closed schema makes them safe.
- [x] Multipart record attachments remain withheld until C-426.
- [x] Every operation composes an absolute, brace-free request to the declared Zendesk host under the
      provider-scoped gate.

## Progress

- 2026-08-02: re-read the pinned Ticketing bytes with `yq -o=json` and `jq`. The document declares
  `ListCustomObjects.include_ui_path` as an optional boolean, not the string recorded by the frozen
  inventory, so it remains a safe caller option. `IncrementalTicketEvents` inherits both
  `support_type_scope` and `include`; both free-form strings are omitted while integer `start_time`
  remains required.
- 2026-08-02: `cargo test -p codewandler-connector-spec --test zendesk_spec_selection --test
  vendored_zendesk_specs -- --nocapture` passed 9 tests. The selection test re-measured the seven
  pre-existing per-operation Flux files against their recorded SHA-256 hashes and found all seven
  byte-identical.
- 2026-08-02: `cargo test -p codewandler-connector-pack --test zendesk_rehearsal -- --nocapture`
  passed its five-case request-composition test. All five reads composed `GET` requests against
  `https://acme.zendesk.com`, with the exact retained integer/boolean query values and no body.
- 2026-08-02: `cargo run -p connector-cli -- diff --provider zendesk` reported `16 artifacts up to
  date (1 provider checked)` after the scoped build. No whole-catalogue artifact was regenerated.

## Notes
- This story writes the same provider/artifacts as C-461 and therefore follows it serially.
