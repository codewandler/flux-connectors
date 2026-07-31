---
id: C-224
title: "`RateLimit` takes a fixed pair, so a header-discovered budget survives only as prose — and two shipped connectors now decline to declare one"
pillar: Spec
status: ready
priority: 3
design:
epic:
areas: [connector-spec]
note: "found by the C-216 implementor 2026-07-31. hubspot declined on tier grounds, discord on discovered-bound grounds. Two independent refusals of the same declaration is the signal the shape is wrong, not that the connectors are lazy"
---

# `RateLimit` cannot express a budget the vendor discloses at runtime

## Goal

Let a connector state a rate limit it genuinely knows the shape of, so the information reaches an
artifact instead of surviving only in a description a machine cannot act on.

## What was measured

`quirks.rate_limit` takes a fixed `requests` / `per_seconds` pair. Two shipped connectors have now
declined to declare one, for **different** reasons, and both recorded why in prose:

| connector | why it declined |
|---|---|
| `hubspot` | the limit is a function of the customer's **tier**, so no single pair is true for all operators |
| `discord` | the limit is **per-route**, with a bucket per major path parameter, and is *discovered* from `X-RateLimit-*` / `Retry-After` response headers |

Discord's is the sharper case. The one published figure — a global 50 requests/second per bot — is
shared across every route, so writing it per-operation would state six allowances that no individual
route actually has. Declaring it would be **less** true than declaring nothing.

Two independent refusals is the point. One connector opting out is a connector's judgement; two
opting out for unrelated reasons is the declaration's shape being too narrow for the domain.

## What the prose costs

The rule currently lives in the connector description and in the write operation's own description,
asserted only by `the_rate_limit_rule_is_stated_where_a_model_reads_it`. That is the right fallback
and it is genuinely useful to a model reading the catalogue — but it reaches no artifact, no
manifest field and no host. Nothing can back off on it, and nothing can check it.

## Acceptance

- [ ] **Failing-first test:** a connector declares a header-discovered budget and it reaches the
      emitted artifact. It cannot be expressed today. Name the test.
- [ ] `RateLimit` grows a variant — or is replaced — that distinguishes at least: a **fixed** pair
      (what exists), a **discovered** budget naming the response headers that carry it, and an
      **unknown-by-tier** budget. Whether that is one enum or a struct with an optional pair is the
      implementor's call; record the reason.
- [ ] A discovered budget names the actual headers (`X-RateLimit-Remaining`, `X-RateLimit-Reset`,
      `Retry-After`) rather than implying a convention. Vendors disagree about these and the
      disagreement is the whole difficulty.
- [ ] `hubspot` and `discord` both stop being exceptions — each declares what it knows, and the
      prose in their descriptions is reduced to what the declaration cannot carry rather than
      duplicating it. Two spellings of one fact is the defect to avoid here.
- [ ] What a consumer is expected to **do** with a discovered budget is stated. A declaration nothing
      acts on is prose with a schema.

## Notes

- Sequencing: this is a `connector-spec` change, so it collides with any story touching the same
  public surface and should run solo or early in a wave.
- Do not let this grow into implementing back-off. Declaring the shape and implementing the retry are
  separate; this story is the first only.
- [C-12](C-12-quirks-as-control-flow.md) is the epic this sits under conceptually — read its position
  on quirks-as-control-flow before choosing the shape, so this does not become a fourth vocabulary.
