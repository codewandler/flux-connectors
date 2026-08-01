---
id: C-447
title: "Decide: is `balance` one contract, or is money separate from metered usage?"
pillar: Spec
status: ready
priority: 3
design: docs/designs/balance-contract.md
epic: balance-contract
areas: [connector-spec]
note: "measured: three vendors already answer 'how much is left' and no two mean the same thing — settled funds (stripe, per-currency list in minor units), prepaid credit (openrouter, a subtraction), metered counts (babelforce, a time series). Splitting is cheap now"
---

# Decide: is `balance` one contract, or is money separate from metered usage?

## Goal

Decide the **shape** — how many contracts the "how much is left?" capability actually is — before any
`Role` variant is proposed. Produce a written answer in
[balance-contract.md](../designs/balance-contract.md).

## The measurement this rests on (2026-08-02)

Three shipped providers already answer the question, and no two return commensurable values:

| operation | returns |
|---|---|
| `stripe-balance-get` | `{available: [{amount, currency}], pending: [...]}` — a **list per currency**, amounts in the currency's **smallest unit** |
| `openrouter-credits-get` | `{data: {total_credits, total_usage}}` — **neither field is the balance**; it is their difference, in vendor credit units |
| `babelforce-task-usage` | a **time series** of task counts at `60s`/`1h`/`1d` — no monetary dimension at all |

Re-measure before quoting: `jq` over `web/public/catalog.json` for operation ids matching
`balance|credit|quota|usage|billing|invoice|subscription|limit`.

## Why the answer is not obviously "one"

- **Money and metered counts are different questions.** "How many dollars can I pay out" and "how
  many inbound calls did I place last hour" share a English word and nothing else.
- **Settled funds and prepaid credit may also differ.** A Stripe balance is money the vendor owes
  *you*; an OpenRouter balance is spend you have pre-purchased *from* them. Direction is opposite.
- **Units are not comparable.** Minor currency units, vendor credit units, and task counts cannot be
  summed, ranked, or thresholded against one another. A contract that returns "a number" invites
  exactly that.

## Why the answer is not obviously "three", either

- A closed set with three near-identical single-member variants is a taxonomy nothing populates,
  which is the trap [C-153](C-153-service-tags.md) records for tags and `provider-roles.md` for roles.
- `provider-roles.md` chose **closed and flat** *"on the reasoning that widening later is cheap;
  narrowing an open system is not."* That reasoning cuts toward starting narrow — perhaps money only —
  rather than toward three.

## Acceptance

- [ ] A decision recorded in [balance-contract.md](../designs/balance-contract.md) with its
      reasoning: how many contracts, what each is named, and what each promises to return.
- [ ] The decision states, for each of the three measured operations, **whether it implements the
      contract, and if not, why not**. "OpenRouter does not implement it" is an acceptable outcome if
      the reason is written down.
- [ ] It states explicitly whether **units** are part of the contract's promise. If they are, the
      currency/minor-unit/credit-unit typing problem gets its own follow-up story rather than being
      solved in passing.
- [ ] It records that this is the **first candidate contract that cuts across tags** (`payments`,
      `ai`, `telephony`) and whether that survives the split — it is the clearest evidence so far
      that roles and tags are genuinely different axes.
- [ ] **No `Role` variant is added in this story.** `connector-contracts.md` §Out of scope refuses
      defining contracts ahead of the mechanism, and that still binds. This produces the shape
      analysis only.
- [ ] Closes as `done` whichever way it goes, including "not worth a contract yet".

## Progress
- (not started)

## Notes
- Depends on nothing; blocks any `balance`-shaped `Role` variant.
- [C-448](C-448-a-contract-cannot-require-a-derived-value.md) is the mechanism half and can be
  answered independently.
