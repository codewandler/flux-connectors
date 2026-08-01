---
id: C-414
title: "Risk and idempotency stated by selector, with silence refusing"
pillar: Spec
status: backlog
priority: 2
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec]
note: "214 of babelforce's 398 operations mutate and no spec publishes risk. Stating each is 214 blocks; deriving from the HTTP method is 214 unverified claims a host reads as a licence"
---

# Risk and idempotency stated by selector, with silence refusing

## Goal
Let one statement declare `risk` and `idempotency` over a matched set, while keeping the rule this
repo has twice legislated: a claim about damage is stated, never guessed.

## Acceptance
- [ ] A `[[patch.select]]` may carry `risk` and `idempotency`, applying to every operation it matched.
- [ ] **Silence on a mutating method refuses the build.** A failing-first test selects a `DELETE` with
      no stated risk and asserts the load fails naming the operation — it must not default to `low`.
- [ ] A per-operation patch overrides a selector's value, and the precedence is the same one C-411
      states.
- [ ] A non-mutating operation may inherit a stated default, because there is no damage claim to get
      wrong — and the asymmetry is documented where an author reads it.
- [ ] `repeatable_because` keeps working exactly as C-186 built it: a selector declaring
      `idempotency = "conditional"` over many mutating operations must still produce a stated
      condition per operation, or refuse. A bulk escape hatch around C-186 is the one outcome this
      story must not produce.

## Progress
- (not started)

## Notes
- The precedent is explicit: `Risk` has no `Default` and `Idempotency`'s doc comment says guessing "is
  how a retry turns one charge into three" (`crates/connector-spec/src/ir.rs:85-115`).
- A default that must be overridden to **lower** risk is safe; one that flatters is not. If a default
  is introduced at all it is the conservative direction only.
