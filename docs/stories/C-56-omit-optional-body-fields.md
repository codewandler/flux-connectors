---
id: C-56
title: Omit an optional body field instead of sending an explicit null
pillar: Codegen
status: ready
priority: 5
design:
epic: connectors-v1
areas: [connector-flux]
note: query params get a `when` guard; body fields do not
---

# Omit an optional body field instead of sending an explicit null

## Goal
Let a connector declare optional request-body fields at all: today an unset one travels as
`{"field": null}`, which many vendors reject or misread, so optional body fields have to be left out
of provider definitions entirely.

## Acceptance
- [ ] An optional body field the caller does not supply is **absent from the payload**, not null.
      `crates/connector-flux/src/op.rs:423` partitions required/optional over `query` only, and the
      file's sole `Node::When` (`:459`) sits inside that branch; `body_tree` (`:620-661`) inserts every
      body field unconditionally. The `when` guard that already exists for query values is the shape to
      mirror.
- [ ] Failing-first: a test asserting an unsupplied optional body field does not appear in the emitted
      payload, red before the change.
- [ ] Nested body paths (C-29's `wire` paths) are covered: omitting a leaf must not leave an empty
      parent object behind, and the rule for that is stated.
- [ ] **The connectors held back by this gap are filled in**, and their stories' exclusions retired:
      OpenAI's `temperature`, `top_p`, `n`, `stop`, `seed`, `response_format`, `tools` and embeddings'
      `dimensions`/`encoding_format`, plus GitHub's `body`/`labels`/`assignees` on issue creation.
- [ ] `freshdesk` already ships twelve optional body fields with this latent behaviour — the change
      either fixes them or the diff says why they were already fine.
- [ ] Regenerated artifacts are committed and the rebuild is a fixed point.

## Progress
- Not started. Filed 2026-07-30 from C-51, where the claim was verified in the emitter source by an
  independent reviewer before this story was written.

## Notes
- This is the highest-fidelity-per-line change available: it unblocks every optional field on every
  provider at once, and it is the reason C-51 ships four deliberately thin operations.
- GitHub's `labels`/`assignees` are the concrete failure case — an explicit `null` there is a 422,
  where a missing key is fine.
