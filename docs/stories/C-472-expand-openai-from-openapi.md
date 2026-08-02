---
id: C-472
title: "Expand OpenAI from its official OpenAPI description"
pillar: Agent
status: done
priority: 13
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [providers, openapi]
note: "preserve 4 operations; add stored response, response-input, file and batch reads"
---

# Expand OpenAI from its official OpenAPI description

## Goal

Make OpenAI spec-backed and add four query-curated visibility operations without sweeping its broad
generation, administration, upload or deletion surfaces into model-visible tools.

## Acceptance

- [x] Vendor the pinned MIT-licensed first-party `openai/openai-openapi` document and a deterministic
      `/v1`-normalized extraction with provenance, scrub and drift tests.
- [x] Failing-first tests pin all four existing operations/Flux bytes and prove the four exact C-468
      selectors are the only additions.
- [x] The response, response-input, file and batch reads retain their official JSON response schemas;
      only numeric limits survive query curation.
- [x] Scoped build/diff and request rehearsal are green.

## Progress

- 2026-08-02 — the failing-first selection test passed all four established Flux hash pins, then
  failed `left: 0, right: 4` for the absent exact patches and `left: 0, right: 1` for the absent spec.
- 2026-08-02 — `scripts/vendor-openai-spec.py --source-file <pinned> --fetched-at
  2026-08-02T11:28:56Z --check` reproduced the committed outputs from the immutable upstream SHA-256
  `ef36175ba6181b9d725a16b1eedcaa75a8a1268124896b10ccc5d9ddf0d543d3`: 182 paths, 288
  operations, 964 example keys scrubbed and 244 referenced components retained.
- 2026-08-02 — `cargo run -p connector-cli -- build --provider openai` planned 11 artifacts and
  wrote the seven expected changed/new provider artifacts; the subsequent scoped diff reported `11
  artifacts up to date (1 provider checked)`.
- 2026-08-02 — the OpenAI selection, connector and rehearsal targets passed 3/3, 7/7 and 1/1. The
  rehearsal target composed all four exact reads, and all four pre-existing operation Flux hashes
  remained byte-identical to the C-468 fence.
