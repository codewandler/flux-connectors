---
id: C-448
title: "A contract cannot require a derived value, and nothing says so"
pillar: Spec
status: ready
priority: 3
design: docs/designs/balance-contract.md
epic: balance-contract
areas: [connector-spec]
note: "found via openrouter: its balance is `total_credits - total_usage`, arithmetic this repository has no way to express — AGENTS.md refuses formulas outright. Conformance can map INPUTS; nothing can derive an OUTPUT, and no design records the limit"
---

# A contract cannot require a derived value, and nothing says so

## Goal

Record, once and in the contracts design, that **a capability contract cannot require a response value
the vendor does not return directly** — and decide what a would-be implementer does instead.

## How it was found

`providers/openrouter.toml:495-498`, written from the vendor's own documentation:

> two numbers, nested under `data`, and **neither of them is the balance**. The vendor documents the
> balance as `total_credits - total_usage`, a subtraction the caller performs, so a consumer reading
> `data.total_credits` as "what is left" would over-read the account by everything ever spent.

A contract promising "the remaining balance" is therefore **unsatisfiable by OpenRouter** — not for
want of data, but because producing the value needs arithmetic. `AGENTS.md` §Flow graph contract:

> **No node ever carries a formula.** … **this repository generates the Flux expression, the author
> never writes one.** `NodeKind::free_text` is the exhaustive tripwire, and there is no `Formula` role
> to classify a new field as.

That refusal is deliberate and this story does **not** propose relaxing it.

## Why it needs writing down

Conformance work to date maps **inputs**: a slot names an operation, and a mapping can rename a
parameter or pin a default so vendor spellings stay off the contract surface.
[member-io-schemas.md](../designs/member-io-schemas.md) composes an `input_schema` — again, inputs.
**Nothing anywhere addresses the output side**, so the first person to write a contract whose
response shape differs from a vendor's will rediscover this the hard way, most likely by inventing a
derivation in prose that no code enforces.

`connector-contracts.md` already warns about the adjacent failure — *"A contract that checks names and
not types is a contract that binds the wrong thing successfully"* — and this is the same hazard one
step further out: a contract that checks types but silently requires a transformation nobody performs.

## The options, none free

- **Contracts promise raw vendor fields**, and the derivation is documented, not enforced. Truthful;
  reduces a contract to discovery, and every caller re-implements the subtraction.
- **flux derives at the contract boundary.** Consistent with `connector-contracts.md`'s split — this
  repository declares conformance, flux owns resolution and dispatch — but a per-vendor formula then
  lives somewhere, and that somewhere needs a name and an owner.
- **A vendor needing derivation does not implement the contract.** Cheapest, and it keeps every rule
  intact; the cost is losing the second implementation that would prove the shape is
  vendor-independent at all.

## Acceptance

- [ ] The limit is stated in [connector-contracts.md](../designs/connector-contracts.md) — the
      substitution design is where a future contract author will look — with the OpenRouter case as
      its worked example and a citation to `AGENTS.md`'s no-formula rule.
- [ ] One of the three options above is chosen, or a fourth is written; the reasoning is recorded and
      the rejected ones are named so they are not re-litigated.
- [ ] The statement is written **generally**, not about `balance` — the next contract to hit this
      will be a different one.
- [ ] **No relaxation of the no-formula rule is proposed.** If the chosen answer needs derivation
      somewhere, it is on flux's side of the split and gets filed on flux's board, in the idiom of
      `channel-bindings-flux-stories.md`.
- [ ] Documentation-only is an acceptable outcome; if nothing lands in a crate, say so rather than
      inventing a test.

## Progress
- (not started)

## Notes
- Independent of [C-447](C-447-decide-balance-shape.md) — that one decides the shape, this one
  decides what the mechanism can promise. Either may land first.
- Related: the input-side mapping question lives with the conformance mechanism; this is deliberately
  its mirror.
