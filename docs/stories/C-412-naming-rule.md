---
id: C-412
title: "A declared naming rule turns operationId into a stable op id"
pillar: Spec
status: in-progress
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
- [x] `[patch.naming]` declares a `rule` and a `prefix`; `[patch.naming.pin]` overrides individual
      operations. A failing-first test asserts `listReportingCalls` → the declared spelling.
      → `provider.rs` `Naming`/`NamingRule`/`kebab`;
      `tests/operation_selection.rs::the_naming_rule_derives_the_declared_spelling`.
      Naming precedence is total and stated once: **`rename`, then a pin, then the rule** —
      `a_pin_overrides_the_rule_and_a_rename_overrides_the_pin`.
- [x] Derived ids satisfy flux-lang's `decl_name` grammar — alphanumerics, `_` and `-` only (C-8). A
      spec whose `operationId` cannot produce a legal name is a reported error naming the operation,
      not a mangled id.
      → `legal_op_id`, which also refuses an empty `-`-separated level because
      `connector_pack::dotted_name` cannot project one. Non-alphanumerics are **passed through**
      rather than substituted, precisely so the result is refused rather than mangled.
      `an_operation_id_that_cannot_produce_a_legal_name_is_reported`.
- [x] **Collisions refuse.** Two operationIds deriving one op id is an error, never last-write-wins.
      → `offer`; the colliding operation is dropped rather than published, so the cause is reported
      once. `two_operation_ids_deriving_one_op_id_refuse` uses the real case — babelforce declares
      `getUser` in `manager-2026-07-10` **and** in `user-2026-06-25`.
- [x] **Stability is asserted, not hoped for.** A test pins the full derived id set for a fixture, so
      an upstream `operationId` rename moves the op id loudly — an op id is what users and models call
      by name and must not drift silently.
      → `the_derived_id_set_is_pinned` writes out all 30 ids `task-automation` derives.
- [x] A pin naming an operationId absent from the spec is a loud error.
      → `check_pins`; `a_pin_naming_an_absent_operation_id_is_refused`.

## Progress
- Landed with C-411 and C-414 as one declaration — all three write `provider.rs`.
- **A pin is keyed by `operationId` alone, which is unique inside one document and nowhere else.** A
  key that two of the connector's documents both declare is refused rather than applied twice (it
  would only collide one step later, with a worse message); the way to name one of them is a
  `[[patch.operations]]` block, which states its `service` and outranks a pin. That is the shape the
  canonical fixture uses for `getUser`.
- Measured against the real documents: the `kebab` rule derives **397 distinct ids from 398
  operationIds** across the five documents, with exactly one collision (`getUser`) and zero
  operationIds that cannot produce a legal name. The nine ids `providers/babelforce.toml` ships need
  nine pins, as the story predicted.
- `rename` stays **required** on a `[[patch.operations]]` block when no `[patch.naming]` rule is
  declared, so every existing provider and every golden error is untouched.

## Notes
- This is the mechanism `docs/designs/connector-pipeline.md` calls for under "Op naming is a public
  contract": ids "must not be derived from volatile spec fields like `operationId` without a pinned
  override". The rule plus pins **is** the pinned override, made bulk.
- The nine ids already shipped in `providers/babelforce.toml` are the compatibility target: C-416
  requires them to come out unchanged, which likely means nine pins.
