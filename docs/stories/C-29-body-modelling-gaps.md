---
id: C-29
title: Close the request-body modelling gaps in the IR
pillar: Spec
status: ready
priority: 7
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec]
note: found by C-17 transcribing real providers · blocks correct write operations
---

# Close the request-body modelling gaps in the IR

## Goal
Let the IR describe the request bodies real vendors actually accept, so a generated write operation
sends a body the API will take rather than a flat approximation of one.

## Acceptance
- [ ] **A body parameter records the JSON path it occupies.** `ParamSet.body` is a flat
      `Vec<Param>` with one `name`, but Zendesk's wire body is
      `{"ticket": {"comment": {"body": …}}}`. Without a path, every Zendesk write emits a flat body
      the API rejects.
- [ ] **A body field can be constant** — always emitted, never in the op signature. Zendesk always
      sends `ticket.safe_update = true`. Declaring it `required` with a JSON Schema `const` (the
      current workaround) leaks it into the signature as a parameter a model must pass.
- [ ] **A free-form object body is expressible** — "the body *is* this schema", not "the body is
      these named fields". Two babelforce operations have `{"type": "object"}` bodies with no
      properties and currently ship with no body parameter at all.
- [ ] **A parameter can carry a wire name distinct from its caller-facing name** (Freshdesk's
      `req_id` → `requester_id`).
- [ ] Every change is **additive** — no existing encoding changes, and C-2's determinism and
      round-trip tests keep passing unchanged.
- [ ] `providers/zendesk.toml` can express its real bodies without recording the shape in a
      `description` string.

## Progress
- (not started)

## Notes
- **Found by C-17 while transcribing the three real providers** — these are not hypotheticals, they
  are the four shapes that blocked writing correct definitions. Each is currently recorded in a
  parameter's `description`, which codegen cannot compile.
- Sequencing: C-9 owns body *emission* and is running now. If C-9 refuses these shapes with a named
  error rather than emitting something plausible-but-wrong, that refusal is the right outcome and
  this story removes the need for it.
- Gap 1 is the load-bearing one. The other three are each expressible as one additive field.
- Two further gaps C-17 found are **not** in this story because they are not about bodies:
  a `base_url` template variable has no declared env binding, and a hand-authored connector cannot
  record provenance without a `[spec]` table. Both deserve their own story.
