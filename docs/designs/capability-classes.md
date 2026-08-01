# Candidate capability classes — what the catalogue actually populates

**Status:** proposed (a register, not a decision) · **Pillar:** Spec · **Epic:** `provider-roles` ·
**Extends:** [provider-roles.md](provider-roles.md), [connector-contracts.md](connector-contracts.md) ·
**Companion:** [balance-contract.md](balance-contract.md)

> Every count below is from one pass over `web/public/catalog.json` on **2026-08-02** — 54 providers,
> **679 operations**. Re-run before quoting; the method is in §How this was measured.

## Why a register

Contract names have so far been proposed one at a time, from intuition, and then measured. Two of
those measurements changed the answer: [C-443](../stories/C-443-per-operation-required-scopes.md)
(per-operation scopes) turned out to have no populated source, and
[balance-contract.md](balance-contract.md) turned out to be three concepts wearing one word. This
register front-loads the measurement so the next contract is chosen from evidence.

**The bar it applies** is C-121's, already stated on that story: *"two roles, not one — a mechanism
validated by a single role is designed around a single case."* Applied to a class rather than to the
mechanism: **a contract needs at least two independent implementations to be worth defining.** One
implementation is a shape fitted to one vendor.

## The headline: the vocabulary is inverted relative to intuition

The AI-shaped classes are the ones the catalogue does **not** have; the well-populated ones are
unglamorous infrastructure.

### Aspirational — 1 operation between them, across all 54 providers

| candidate | operations | providers |
|---|---|---|
| `agent_memory` | **0** | 0 |
| `vector_store` | **0** | 0 |
| `image_generation` | **0** | 0 |
| `transcription` / `speech_synthesis` | **0** | 0 |
| `embeddings` | 1 — `openai-embeddings-create` | 1 |

None clears the two-implementation bar; four of the five have no implementation at all. That does not
make them wrong as *future* vocabulary — it makes defining them now an exercise in fitting a shape to
zero vendors. The honest place for them is a "wanted" list that a new provider can satisfy, not the
closed enum.

**Where they would come from, if they came:** `openai` already has an embeddings operation and could
gain images; Anthropic's Managed Agents surface has memory stores (see
[anthropic-managed-agents.md](anthropic-managed-agents.md), and note it is charter-gated by
[C-444](../stories/C-444-decide-managed-agents-charter.md)); a vector store would most plausibly
arrive via `supabase` (pgvector) or a vendor not yet in the fleet. Each is a provider story first and
a contract second, in that order.

### Populated — the classes nobody named

| candidate | ops | providers | the operations |
|---|---|---|---|
| **`incident`** | 10 | **3** | `datadog-incident-{get,list}`, `pagerduty-incident-{get,list,acknowledge,resolve}`, `statuspage-incident-{get,list,create,update}` |
| `ticketing` | ~18 | 4+ | `zendesk`, `freshdesk`, `front`, `intercom` — already named as a proposed role in `provider-roles.md`, never implemented |
| `email_send` | 3 | **3** | `postmark-email-send`, `resend-email-send`, `google-gmail-message-send` |
| `search` | 4 | **4** | `algolia-index-search`, `dropbox-search`, `notion-search`, `zendesk-ticket-search` |
| `calendar` | ~9 | 4 | `google`/`calendar`, `microsoft_graph`/`calendar`, `calendly`, `zoom` |
| `crm_contact` | ~11 | 5 | `hubspot`, `salesforce`, `intercom`, `freshdesk`, `shopify` |
| `file_store` | ~9 | 4 | `box`, `dropbox`, `google`/`drive`, `microsoft_graph`/`files` |
| `feature_flag` | 3 | 1 | `launchdarkly` only — below the bar |

**`incident` is the strongest candidate in the catalogue, and by a distance.** Three independent
vendors expose the *same verb set* — `get` and `list` in all three — plus a lifecycle surface
(`acknowledge`/`resolve`, `create`/`update`). Compare the currently shipped `llm_catalogue`: also
three vendors, but a single `list` slot. `incident` is the first candidate where substitution —
"page whoever is on call, whichever vendor this tenant uses" — is a real user story rather than a
discovery convenience.

## The trap `search` walks straight into

`search` looks excellent on the count — four providers, one clean operation each — and it is the one
to be most careful with. The four search **different kinds of object**: an Algolia index record, a
Dropbox file, a Notion page, a Zendesk ticket. The *call* is substitutable; the *result* is not.

That is [balance-contract.md](balance-contract.md)'s finding arriving in a second place, which is
what makes it a pattern rather than an oddity: **a contract can be satisfiable on its inputs and
meaningless on its outputs.** A `search` contract would need to say what a result *is* before it says
anything useful, and the answer differs per vendor. Any register entry for `search` carries this
warning or it is misleading.

`zendesk-ticket-search` also carries a live defect — `AGENTS.md` §Intentional gaps records it as
non-functional because query values are not percent-encoded — so one of the four implementations does
not currently work.

## Not every class should be a Role

The repository already has **three** mechanisms for "this operation plays a known part", and only one
of them is `roles`. Choosing the mechanism is a separate question from choosing the name:

| mechanism | shape | populated |
|---|---|---|
| `roles` on a service | a checked shape with required members, closed enum | **1 variant**, 1 provider |
| `verify` on a connector | a named reference to one operation | **28 providers** |
| `produces_credential` / `credential_response` | a marked operation | C-136 / C-430 |

**`verify` is already a universal single-slot contract, implemented as a field.** It says "this
operation proves the arrangement works", every consumer knows what to do with it, and 28 providers
declare one — nearly thirty times the adoption of the role mechanism. A candidate class with exactly
one slot and no parameters is probably a `verify`-shaped field, not a `Role`. That comparison is
worth making explicitly before adding any variant.

Also note `tags` ([C-153](../stories/C-153-service-tags.md), landed) already answers *"what kind of
thing is this?"* for all 54 providers. Several entries above — `calendar`, `file_store`, `crm_contact`
— may be tags that describe a domain rather than roles that promise a callable shape. The test is
`provider-roles.md`'s: can you name the required members? If not, it is a tag.

## How this was measured

One pass over `web/public/catalog.json` matching operation ids per candidate, then a second pass
excluding `babelforce` and printing the ids for manual reading.

**Two caveats that matter, and both cut toward over-counting:**

1. **`babelforce` was excluded from the per-class ids.** It compiles 391 operations of which 9 are
   exposed, so including it inflated `ticketing`, `calendar`, `file_store` and `feature_flag` with
   operations no model can reach. The counts above are the babelforce-excluded ones.
2. **`expose` is not in `catalog.json`**, so the remaining counts still cannot be filtered to the
   model-facing set. They are upper bounds. Anyone acting on this register should re-measure against
   the IR, where `Operation::expose` is readable.

## What this register is not

- **Not a decision.** No `Role` variant is proposed here.
  `connector-contracts.md` §Out of scope still refuses defining contracts ahead of the mechanism, and
  `provider-roles.md`'s closed-and-flat choice still stands.
- **Not a reason to ship providers.** Shipping a vendor to populate a contract is backwards; the
  register records what exists, not what would be convenient.
- **Not a ranking of importance.** It ranks *evidence*. A class with one implementation may still
  matter more to a user than one with four.
