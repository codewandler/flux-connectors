---
id: C-106
title: Ship the Stripe connector
pillar: Spec
status: ready
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "the second vendor in C-60's HMAC matrix — inbound-events.md already tabulates Stripe-Signature with its t=/v1= pairs and tolerance, and nothing has ever exercised that row"
---

# Ship the Stripe connector

## Goal
Payments, and the second real vendor behind the webhook verification matrix.

## Acceptance
- [ ] A curated operation set — customers, charges or payment intents, refunds. Not the whole API.
- [ ] **Risk and idempotency are the point here, not paperwork.** A charge is a `high`-risk write; a
      refund is arguably `destructive`. Stripe issues idempotency keys, so `conditional` is the
      honest value where one applies — and the file records which operations that is and why.
- [ ] Auth: a bearer secret key. The `[[config]]` field says plainly that a test key and a live key
      are different values, because that is the mistake this connector will actually cause.
- [ ] `[[events]]` and a `webhook` `[[channels]]` binding carrying the **published** Stripe scheme —
      `Stripe-Signature`, HMAC-SHA256, hex, the `t=`/`v1=` pair, `{timestamp}.{body}` signed, with a
      tolerance. This is the row `docs/designs/inbound-events.md` tabulates and nothing has exercised.
- [ ] A per-provider contract test in `crates/connector-flux/tests/`, following the shipped pattern.

## Progress
- Not started.

## Notes
- Stripe's signature header packs two values into one header (`t=…,v1=…`), which `HmacSpec.prefix`
  may not express — it assumes a single literal prefix. **If it cannot be expressed, that is a
  finding worth more than the connector**: it means the matrix needs a parse rule, and it should be
  filed rather than worked around.
