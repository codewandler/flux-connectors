---
id: C-6
title: Build the patch/overlay layer
pillar: Spec
status: backlog
priority:
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec]
note: the real bet — if patching is harder than hand-writing, the thesis fails
---

# Build the patch/overlay layer

## Goal
Let a provider TOML select, rename, correct, and annotate what ingest extracted, deterministically —
so a 400-endpoint vendor spec becomes a curated, correctly-typed set of operations.

## Acceptance
- [ ] **Selection is opt-in**: only operations a provider explicitly selects reach the IR. A spec
      with 400 endpoints must not yield 400 tools.
- [ ] A patch can rename an operation to a stable op id, override `risk`/`idempotency`/`description`,
      correct a parameter's type or requiredness, and add a parameter the spec omitted.
- [ ] Quirks attach per operation: pagination style, rate limit, error envelope.
- [ ] Merge order is fixed and total (**spec → patch → validate**), and a test asserts the same
      inputs always produce byte-identical IR.
- [ ] A patch that targets an operation absent from the spec is a loud error, not a silent no-op —
      that is how config rots.
- [ ] Zendesk's real spec plus a patch set reproduces the operation set of
      `../flux/plugins/zendesk/src/main.rs`.

## Progress
- (not started)

## Notes
- This is the highest-risk story in the epic. If patching a bad vendor spec turns out harder than
  hand-writing the integration, the whole premise needs revisiting — surface that early rather than
  working around it.
- Op ids are a **public contract** (users and models call them). They must be stable across
  regeneration and must not derive from volatile spec fields like `operationId` without a pinned
  override.
