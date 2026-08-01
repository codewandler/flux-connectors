---
id: C-409
title: "The spec front-end, proven by retiring manager-sdk (epic)"
pillar: Spec
status: ready
priority: 1
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec, connector-cli, connector-flux, providers]
note: "EPIC — the `[spec]` half of the pipeline was designed in C-2 and never built; all 53 providers are hand-authored and seam.rs:160 refuses a spec-backed one outright. babelforce forces it: 397 operations across 5 documents, which nobody is hand-authoring"
---

# The spec front-end, proven by retiring manager-sdk (epic)

## Goal
Make `[spec]` + patches a working front-end — ingest, overlay, and the bulk declarations that let one
statement cover many operations — and prove it by describing all 397 babelforce manager operations in
this repo, so `~/babelforce/projects/manager/manager-sdk` can be archived.

## Acceptance
- [ ] A provider pointing `[spec]` at a vendored document builds — `seam.rs:160`'s refusal is deleted,
      not worked around ([C-4](C-4-openapi-ingest.md)).
- [ ] One connector ingests **many** documents, one service each ([C-410](C-410-many-spec-documents.md)).
- [ ] `securitySchemes` and per-operation `security` reach the IR ([C-5](C-5-auth-extraction.md)).
- [ ] The overlay applies deterministically over ingest — spec → patch → validate
      ([C-6](C-6-overlay-layer.md)).
- [ ] **Four bulk declarations exist**, so 397 operations do not cost 397 blocks: a set selector
      ([C-411](C-411-selector-matches-a-set.md)), a naming rule ([C-412](C-412-naming-rule.md)),
      an exposure tier ([C-413](C-413-callable-without-being-a-tool.md)), and risk/idempotency by
      selector with silence refusing ([C-414](C-414-risk-by-selector.md)).
- [ ] The five babelforce documents are vendored, scrubbed and provenanced
      ([C-415](C-415-vendor-babelforce-specs.md)).
- [ ] **The nine shipped babelforce operations come out of the spec route byte-identical**
      ([C-416](C-416-reproduce-the-nine.md)) — the migration safety net and C-6's real test.
- [ ] babelforce reaches manager-sdk's 397 with a coverage gate that fails on regression
      ([C-417](C-417-widen-to-full-coverage.md)).
- [ ] The retirement is stated, with the gaps that block it resolved or scoped out
      ([C-418](C-418-retire-manager-sdk.md)).
- [ ] Drift against the upstream documents is detectable ([C-14](C-14-fetch-and-drift-check.md)).

## Progress
- 2026-08-01 — Epic filed. Owner decided the *retire it* reading: consumers reach babelforce through
  `connectors-api` or flux ops, and this repo grows **no** language-client emitters. The two rejected
  readings are recorded in the design's Alternatives.

## Notes
- The schema for `[spec]`/`[[patch.operations]]` has been landed and unused since C-3
  (`crates/connector-spec/src/provider.rs:73-176`), with golden errors already written. This epic
  builds the machinery behind it, it does not design the file format from scratch.
- **Selection stays opt-in.** C-411 widens what one statement selects; it must not introduce `hide`
  or make anything default-selected.
- Three gaps block the *retirement claim*, not the ingest work: multipart is inexpressible
  (`BodyEncoding` is `Json | Form`), form bodies wait on upstream flux `L-101`, and 23 operations
  carry no description. See the design's "What retiring manager-sdk actually requires".
