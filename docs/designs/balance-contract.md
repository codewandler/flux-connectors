# Balance is not one contract — settled funds, prepaid credit, and metered usage

**Status:** proposed — **the shape question is open and §What the fleet actually returns decides it** ·
**Pillar:** Spec · **Epic:** `balance-contract` ·
**Extends:** [provider-roles.md](provider-roles.md), [connector-contracts.md](connector-contracts.md) ·
**Companion:** [member-io-schemas.md](member-io-schemas.md)

## Why

The generalized-provider vocabulary so far names things a service *is* or *holds* — a secret store, a
model catalogue, a payments surface. "How much is left?" is a different axis, and the instinct that
it belongs in the vocabulary is right: **the capability is already populated in the shipped
catalogue, by three vendors, without anything naming it.**

It is also the **first candidate contract that cuts across tags**, which is the roles-vs-tags
distinction ([C-153](../stories/C-153-service-tags.md)) doing real work rather than being asserted.
`stripe` is tagged `payments`, `openrouter` is `ai`, `babelforce` is `telephony`. No tag groups them;
a role would. That is exactly the split `provider-roles.md` draws — a tag says what *kind* of thing a
service is, a role says what it can *do*.

And it is the natural `verify` operation for a prepaid vendor: `providers/openrouter.toml:70` records
that `openrouter-credits-get` **is** that connector's verification operation, chosen over the models
list because it is account-scoped and actually fails on a bad key.

## What the fleet actually returns — measured 2026-08-02

From `web/public/catalog.json`, the operations whose ids match
`balance|credit|quota|usage|billing|invoice|subscription|limit`:

| operation | vendor tag | response shape |
|---|---|---|
| `stripe-balance-get` (`GET /v1/balance`) | `payments` | `{available: [{amount, currency}], pending: [...], livemode, object}` — **a list, one entry per currency**, each `amount` in that currency's **smallest unit** |
| `openrouter-credits-get` (`GET /api/v1/credits`) | `ai` | `{data: {total_credits, total_usage}}` — **neither field is the balance** |
| `babelforce-task-usage` (`GET /api/v3/tasks/usage`) | `telephony` | a **time series** of task counts with a `rate` of `60s`/`1h`/`1d` — not money at all |
| `babelforce-task-usage-types` | `telephony` | an array of task-type strings (`babelforce.call.inbound`, …) |

**Three vendors, three incommensurable concepts:**

1. **Settled funds** — money you have, per currency, in minor units, split across available and
   pending.
2. **Prepaid credit** — a vendor-scoped unit that is not a currency, reported as two cumulative
   totals.
3. **Metered consumption** — counts over time, with no monetary dimension whatsoever.

Calling all three `balance` and giving them one contract would produce a capability that resolves and
then means something different per vendor. That is the failure mode `provider-roles.md` names in its
own words — *"a second vendor filling the same shape was a coincidence rather than a contract"* —
arriving from the opposite direction.

## The finding that makes this worth an epic: a connector cannot compute

`providers/openrouter.toml:495-498` records it precisely, and it was found by reading the vendor's
own documentation rather than assumed:

> two numbers, nested under `data`, and **neither of them is the balance**. The vendor documents the
> balance as `total_credits - total_usage`, a subtraction the caller performs, so a consumer reading
> `data.total_credits` as "what is left" would over-read the account by everything ever spent.

So a contract promising *"read the remaining balance"* is **unsatisfiable by OpenRouter**, not because
the data is missing but because producing it requires **arithmetic**, and this repository has no
arithmetic. `AGENTS.md` §Flow graph contract is unambiguous:

> **No node ever carries a formula.** … every rejection in this repository's history was an
> *expression* language … **this repository generates the Flux expression, the author never writes
> one.** `NodeKind::free_text` is the exhaustive tripwire, and there is no `Formula` role to classify
> a new field as.

This is a **new limit on the conformance mechanism**, and it is the epic's most transferable result.
A declared-conformance field can rename a parameter and pin a default — it maps *inputs*. Neither
that nor `member-io-schemas.md`'s composed `input_schema` can **derive an output value**. `balance` is
the first candidate contract where satisfying it needs a computed response, and the honest options
are all uncomfortable:

- **The contract returns the vendor's raw fields** and names the derivation in prose — truthful, but
  then it is a discovery role, not a substitutable contract, and every caller re-implements the
  subtraction.
- **flux performs the derivation** at the contract boundary — consistent with
  `connector-contracts.md`'s split (this repository declares conformance; flux owns resolution and
  dispatch), but it puts a per-vendor formula somewhere, and that somewhere needs naming.
- **OpenRouter simply does not implement the contract** — cheapest and possibly correct, but it
  removes the second implementation that would have proved the shape was vendor-independent at all.

## What this epic must decide, in order

1. **Is `balance` one contract or three?** The measurement says at least two axes are real (money vs
   metered) and possibly three (settled vs prepaid). Splitting is cheap now and expensive later —
   `provider-roles.md` chose closed and flat *"on the reasoning that widening later is cheap;
   narrowing an open system is not."*
2. **Can a contract require a derived value at all?** If not, say so once, in the contracts design,
   so the next contract does not rediscover it.

## Deliberately not decided here

- **Any concrete `Role` variant.** `connector-contracts.md` §Out of scope refuses defining contracts
  ahead of the mechanism, and that still binds. This epic produces the *shape analysis*; the variant
  lands with whichever conformance mechanism ships.
- **Currency and unit modelling.** `amount` in minor units, multi-currency lists, and vendor credit
  units are a typing problem this repository has never had. If the answer to (1) is "money is one
  contract", that problem becomes real and gets its own story — do not solve it in passing.
- **Widening the fleet to find more implementations.** Shipping a vendor to justify a contract is
  backwards.

## Stories

Seeded with this design; see the board under the `balance-contract` epic.
