---
id: C-414
title: "Risk and idempotency stated by selector, with silence refusing"
pillar: Spec
status: in-progress
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
- [x] A `[[patch.select]]` may carry `risk` and `idempotency`, applying to every operation it matched.
      → `OperationSelector::risk`/`idempotency`;
      `tests/operation_selection.rs::a_selector_states_risk_and_idempotency_for_the_set`.
- [x] **Silence on a mutating method refuses the build.** A failing-first test selects a `DELETE` with
      no stated risk and asserts the load fails naming the operation — it must not default to `low`.
      → `compose`; `silence_on_a_mutating_method_refuses` asserts the refusal names `deleteAgent`
      and the `DELETE` that made silence unacceptable.
- [x] A per-operation patch overrides a selector's value, and the precedence is the same one C-411
      states.
      → field by field, stated once on `Patch`. `a_block_overrides_a_selectors_risk` shows one
      operation taking the block's `destructive` while its neighbours keep the selector's `high`.
- [x] A non-mutating operation may inherit a stated default, because there is no damage claim to get
      wrong — and the asymmetry is documented where an author reads it.
      → a read a **selector** matched takes `low`/`idempotent` when nothing states otherwise; this is
      the only default in the overlay. Documented on `OperationSelector::idempotency` (rustdoc) and
      in `schema/provider-toml.schema.json`, which is what an author's editor shows.
      `a_read_may_go_unstated`.
- [x] `repeatable_because` keeps working exactly as C-186 built it: a selector declaring
      `idempotency = "conditional"` over many mutating operations must still produce a stated
      condition per operation, or refuse. A bulk escape hatch around C-186 is the one outcome this
      story must not produce.
      → **there is no key for it on a selector**, so a condition cannot be stated in bulk at all;
      every matched write arrives with `repeatable_because: None` and
      `validate_repeatability_condition` refuses it by name.
      `a_bulk_conditional_still_owes_a_condition_per_operation`.

## Progress
- Landed with C-411 and C-412 as one declaration — all three write `provider.rs`.
- **The asymmetry, precisely.** The read default belongs to *selection*, not to the operation: a
  `[[patch.operations]]` block is a statement about one operation and still states both, exactly as
  before, so no existing provider, test or golden error moves. A selector is a statement about a set
  that may mix methods, and requiring it to restate `risk = "low"` for every read would push authors
  back toward per-operation blocks — which is the boilerplate this story exists to remove. `low` and
  `idempotent` are not flattering defaults for a `GET`; they are the only values a read can have
  (`Idempotency::Idempotent`'s own doc: "reserve it for operations that change nothing").
- **`conditional` was deliberately not made reachable through the spec route.** `OperationPatch`
  carries no `repeatable_because` either, so today a selected write declaring `conditional` always
  refuses. Adding that key would be the escape hatch this story names as the one outcome to avoid,
  and it is a decision that deserves its own story rather than a field slipped in beside a bulk
  selector — see this story's `ADJACENT` note in the handoff.
- Measured: 214 of the 398 operations mutate, as the story states. The canonical fixture states risk
  for all of them in **8 selector blocks**.

## Notes
- The precedent is explicit: `Risk` has no `Default` and `Idempotency`'s doc comment says guessing "is
  how a retry turns one charge into three" (`crates/connector-spec/src/ir.rs:85-115`).
- A default that must be overridden to **lower** risk is safe; one that flatters is not. If a default
  is introduced at all it is the conservative direction only.
