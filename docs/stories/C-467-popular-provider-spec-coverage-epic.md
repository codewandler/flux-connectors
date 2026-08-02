---
id: C-467
title: "Five popular providers gain curated coverage from first-party API specs (epic)"
pillar: Agent
status: in-progress
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [providers, openapi]
note: "EPIC — GitHub, Stripe, Microsoft Graph, OpenAI and Twilio; exact selectors, no wholesale exposure"
---

# Five popular providers gain curated coverage from first-party API specs

## Goal

Increase real callable coverage for five widely used existing connectors while replacing hand-copied
API assumptions with pinned, reproducible first-party OpenAPI evidence.

## Acceptance

- [x] C-468 freezes exact sources, hashes, operation ids and representability decisions before fan-out.
- [x] C-469 through C-473 each vendor a first-party spec, preserve their existing published surface
      and add at least four reviewed, callable operations.
- [x] C-474 integrates the five disjoint changes, regenerates all whole-catalogue artifacts and proves
      the complete Rust and web gates.
- [x] The public catalogue distinguishes spec-backed provenance without exposing internal planning.
- [ ] Release handling follows the repository contract: one integrated release for this existing-
      provider wave; every genuinely new provider in later work is released immediately after it lands.

## Progress

- 2026-08-02: baselines re-measured from `web/public/catalog.json`: GitHub 5, Stripe 8,
  Microsoft Graph 8, OpenAI 4, Twilio 5.
- 2026-08-02: official OpenAPI 3 source families confirmed for all five; Slack excluded because its
  official description is Swagger 2.0 and the current ingest refuses Swagger.
- 2026-08-02: HubSpot was removed after its first-party spec repository and documents proved to
  carry no redistribution license; OpenAI's official MIT-licensed OpenAPI 3.1 source replaced it.
- 2026-08-02: integrated counts re-measured from `web/public/catalog.json`: GitHub 9, Stripe 12,
  Microsoft Graph 12, OpenAI 8, and Twilio 9. Each provider contributes exactly four new
  spec-backed operations, the public catalogue carries per-operation provenance, and the complete
  Rust and Node gates are green. The release checkbox remains open until v0.14.0 is published.
