---
id: C-412
title: "A declared naming rule turns operationId into a stable op id"
pillar: Spec
status: backlog
priority: 2
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec]
note: "op ids are a public contract, which is why `rename` exists — but 397 renames is the other half of the boilerplate. Declare the rule once, pin the exceptions, and refuse collisions"
---

# A declared naming rule turns operationId into a stable op id

## Goal
Derive op ids from `operationId` through one declared, deterministic rule with pinned exceptions — so
naming stays a public contract without costing a line per operation.

## Acceptance
- [ ] `[patch.naming]` declares a `rule` and a `prefix`; `[patch.naming.pin]` overrides individual
      operations. A failing-first test asserts `listReportingCalls` → the declared spelling.
- [ ] Derived ids satisfy flux-lang's `decl_name` grammar — alphanumerics, `_` and `-` only (C-8). A
      spec whose `operationId` cannot produce a legal name is a reported error naming the operation,
      not a mangled id.
- [ ] **Collisions refuse.** Two operationIds deriving one op id is an error, never last-write-wins.
- [ ] **Stability is asserted, not hoped for.** A test pins the full derived id set for a fixture, so
      an upstream `operationId` rename moves the op id loudly — an op id is what users and models call
      by name and must not drift silently.
- [ ] A pin naming an operationId absent from the spec is a loud error.

## Progress
- (not started)

## Notes
- This is the mechanism `docs/designs/connector-pipeline.md` calls for under "Op naming is a public
  contract": ids "must not be derived from volatile spec fields like `operationId` without a pinned
  override". The rule plus pins **is** the pinned override, made bulk.
- The nine ids already shipped in `providers/babelforce.toml` are the compatibility target: C-416
  requires them to come out unchanged, which likely means nine pins.
