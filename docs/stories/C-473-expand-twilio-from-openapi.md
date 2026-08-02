---
id: C-473
title: "Expand Twilio from its official API v2010 OpenAPI description"
pillar: Agent
status: done
priority: 14
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [providers, openapi]
note: "preserve 5 operations; add exact recording, usage and conference reads; defer unsafe forms"
---

# Expand Twilio from its official API v2010 OpenAPI description

## Goal

Make Twilio spec-backed and add useful read coverage without disguising calls or messages as
ordinary HTTP writes.

## Acceptance

- [x] Vendor the pinned first-party `twilio/twilio-oai` API v2010 document with deterministic scrub,
      provenance and drift tests.
- [x] Failing-first tests pin all five existing operation identities/Flux bytes and prove at least
      four exact C-468 selectors are added without a sweep.
- [x] The exact recording list/get, usage-record list and conference list operations land; message
      and call creation remain documented deferrals until structured form encoding exists, and still
      require high risk plus `send_external` when later implemented.
- [x] Account SID path binding and Basic auth remain host-configuration/credential driven rather than
      caller-chosen authority.
- [x] Scoped build/diff and request rehearsal are green.

## Progress

- 2026-08-02: Failing-first specification, Flux and rehearsal tests exposed the C-475 prerequisite:
  a Basic username reused as a request pin was falsely rejected as a shared slot, emitted an
  endpoint-shaped placeholder, and could not resolve through the runtime username address.
- 2026-08-02: `python3 scripts/vendor-twilio-spec.py --check --fetched-at
  2026-08-02T11:34:37Z` checked the pinned 121-path/197-operation source, with 967 example keys
  scrubbed and 7 referenced components retained.
- 2026-08-02: `cargo test -p connector-flux --test username_path_pin --test twilio_connector --test
  algolia_connector --no-fail-fast`, the focused `connector-pack` rehearsal tests, and
  `connector-spec`'s omission/Twilio-selection tests all passed: 34 tests in the named binaries.
- 2026-08-02: `cargo run -p connector-cli -- build --provider twilio` reported 12 artifacts up to
  date; `cargo run -p connector-cli -- diff --provider twilio` reported `12 artifacts up to date (1
  provider checked)`.
