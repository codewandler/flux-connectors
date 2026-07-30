---
id: C-106
title: Ship the Stripe connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "the second vendor in C-60's HMAC matrix — inbound-events.md already tabulates Stripe-Signature with its t=/v1= pairs and tolerance, and nothing has ever exercised that row"
---

# Ship the Stripe connector

## Goal
Payments, and the second real vendor behind the webhook verification matrix.

## Acceptance
- [x] A curated operation set — customers, charges or payment intents, refunds. Not the whole API.
- [x] **Risk and idempotency are the point here, not paperwork.** A charge is a `high`-risk write; a
      refund is arguably `destructive`. Stripe issues idempotency keys, so `conditional` is the
      honest value where one applies — and the file records which operations that is and why.
- [x] Auth: a bearer secret key. The `[[config]]` field says plainly that a test key and a live key
      are different values, because that is the mistake this connector will actually cause.
- [ ] `[[events]]` and a `webhook` `[[channels]]` binding carrying the **published** Stripe scheme —
      `Stripe-Signature`, HMAC-SHA256, hex, the `t=`/`v1=` pair, `{timestamp}.{body}` signed, with a
      tolerance. This is the row `docs/designs/inbound-events.md` tabulates and nothing has exercised.
      **Half done: the four `[[events]]` ship; the binding is blocked on C-141.** See Progress.
- [x] A per-provider contract test in `crates/connector-flux/tests/`, following the shipped pattern.

## Progress

Shipped as `providers/stripe.toml` with `crates/connector-flux/tests/stripe_connector.rs` (8 tests).
Scoped gate green: `build --provider stripe` writes 11 artifacts, `diff --provider stripe` reports no
drift, and the workspace leaves exactly the eight whole-catalogue tests red that AGENTS.md tabulates
for a new provider. Four of the five Acceptance items hold; the webhook binding does not, for the
reason the story's own Notes predicted.

### Finding 1 — the pipeline cannot send a form-encoded body, and that is not a Stripe problem

**This is worth more than the connector.** Stripe's API parses `application/x-www-form-urlencoded`
and nothing else — that is the first thing its reference says about how a request is made, and it is
why every official library serializes `metadata[order_id]=6735` rather than JSON. This pipeline sends
exactly one media type, and it is a constant:

```
crates/connector-flux/src/op.rs:144   const JSON_MEDIA_TYPE: &str = "application/json";
crates/connector-flux/src/op.rs:553   body.push(bind_string(CONTENT_TYPE, JSON_MEDIA_TYPE));
```

The second line runs unconditionally whenever an operation declares any body. There is no
`content_type`, no `encoding`, no `body_format` key anywhere in the IR — not on the provider, not on
the operation, not on the `ParamSet` — so the fact cannot even be *declared*, let alone emitted. A
Stripe operation with a body field therefore sends a document Stripe does not parse, and Stripe
answers `400 Missing required param: <field>`. Loud, which is better than zendesk's 200-and-ignore,
but still a connector that cannot write.

**It will hit every form-encoded vendor**, not only Stripe: the OAuth2 token endpoints every provider
publishes, Twilio, Mailgun, PayPal's classic API. Stripe is simply the first one this repository
tried to ship, and the one where the cost is visible, because what it forbids is "create anything".
Closing it needs a request-encoding axis on the IR plus a form encoder in flux; it is additive rather
than a reshape. **Filed for its own story** — this story worked inside it rather than around it.

The selection rule that follows is the connector's whole shape: **an operation ships only if it
addresses everything it needs in the path.** Consequences, each recorded at its site in the TOML:

- `POST /v1/refunds`, the current canonical refund endpoint, takes its subject (`charge` /
  `payment_intent`) as a *body* parameter. The charge-nested `POST /v1/charges/{charge}/refunds` is
  the same operation with its subject in the path, so this connector refunds **by charge**.
- Capture and refund act on the **full** amount, because `amount` is a body parameter. Each
  `description` says so rather than leaving a model to discover it by refunding everything.
- Every `create` and `update` is excluded outright.

`stripe_connector.rs::no_stripe_operation_sends_a_request_body` asserts the absence on the IR *and*
on the emitted text, so a `body_schema` cannot reintroduce it by another route.

### Finding 2 — `conditional` is earned by a required header, which is stricter than Stripe

`Idempotency::Conditional` is documented as "idempotent only under a condition the caller supplies
(e.g. an idempotency key)". Stripe's keys are exactly that mechanism: a `POST` carrying
`Idempotency-Key: <string>` is replayed from Stripe's cache for 24 hours rather than re-executed.

Stripe treats the key as **optional**. This connector declares it **required** on all three writes,
and the reasoning belongs in the file rather than only here: a connector that declared `conditional`
while leaving the key optional would tell flux a retry is sound *while permitting the request that
makes it unsound*. The cost of that error is a customer captured or refunded twice; the cost of this
choice is one more argument on the tool contract. It is also the first `params.header` in the shipped
fleet — every earlier candidate (github's `Accept`, openrouter's attribution headers) was a
*constant*, which the field cannot express (C-52).

Emitted shape:

```flux
response = http.request(headers: { "Idempotency-Key": idempotency_key }, method: "POST", url)
```

### The operation set — 8 of some 450

| id | method + path | risk | idempotency |
|---|---|---|---|
| `stripe-balance-get` | `GET /v1/balance` | low | idempotent |
| `stripe-customer-get` | `GET /v1/customers/{customer}` | low | idempotent |
| `stripe-charge-get` | `GET /v1/charges/{charge}` | low | idempotent |
| `stripe-payment-intent-get` | `GET /v1/payment_intents/{payment_intent}` | low | idempotent |
| `stripe-refund-get` | `GET /v1/refunds/{refund}` | low | idempotent |
| `stripe-payment-intent-capture` | `POST …/{payment_intent}/capture` | **high** | **conditional** |
| `stripe-payment-intent-cancel` | `POST …/{payment_intent}/cancel` | **high** | **conditional** |
| `stripe-charge-refund-create` | `POST /v1/charges/{charge}/refunds` | **destructive** | **conditional** |

`verify = "stripe-balance-get"` — parameterless, read-shaped, and it also reports which key mode the
connection is in.

The refund is `destructive` because money leaves the account, the customer's bank is told, Stripe's
fee on the original charge is not returned, and Stripe publishes no un-refund. The capture is `high`
because it turns an authorization into a real charge on a real card; `medium` would wave that past a
reviewer. The cancel is `high` and not `destructive` — irreversible, but nothing was taken.

**Deliberately excluded:** every `list` endpoint (Stripe filters with bracketed nested keys,
`created[gte]=…`, and pages with an opaque `starting_after` id — C-30 percent-encodes nothing), every
create and update (Finding 1), partial amounts, the whole of Billing, Connect, Issuing, Terminal,
Radar, Tax, Identity, Checkout and Payment Links, and every `DELETE`.

**Not declarable:** the `Stripe-Version` header, which pins response shape by date. It is a
*constant*, which `params.header` emits as a caller-overridable argument (C-52/C-55), so every
response is shaped by whatever version the operator's account is pinned to.

**Not declared:** `rate_limit`. Stripe publishes 100 read and 100 write operations per second in live
mode and 25 of each in test mode, and the same connector serves both — the mode is a property of the
secret key, a value this repository never sees. One `requests`/`per_seconds` pair cannot say
"whichever mode this key is in", so the numbers live in the TOML's header comment.

### The webhook binding is blocked on C-141, and stating it is the point

The four `[[events]]` ship — `payment_intent.succeeded`, `payment_intent.payment_failed`,
`charge.refunded`, and `charge.dispute.created` (`default = false`: a dispute starts a clock and
costs a fee whatever the outcome, so it is not a routine notification). Each schema describes the
Stripe **Event envelope**, since the resource sits at `data.object` and the discriminator is `type`.

There is **no `[[channels]]` binding**, and that is this story's second finding rather than an
omission. `docs/designs/inbound-events.md:58` tabulates Stripe's row and all four of its `HmacSpec`
values — sha256, hex, `{timestamp}.{body}`, a tolerance — are expressible. **The header is not:**

```
Stripe-Signature: t=1614556800,v1=5257a869e7…,v0=6ffbb59b23…
```

`HmacSpec.header` names a whole header and `HmacSpec.prefix` is a single literal, so the digest
cannot be taken out of that list; `HmacSpec.timestamp` is a `Selector` addressing a whole header, so
the `t=` component cannot be reached; and Stripe sends **more than one `v1` during a secret
rotation**, of which a verifier must accept any. C-60 reached this already —
`crates/connector-spec/tests/verification_conformance.rs` declares Stripe `cannot verify` rather than
pretending — and C-141 owns the extraction axis.

Both shortcuts were refused, and each would have built, shipped and passed every artifact check:

1. `verification = "none"` declares a public endpoint accepting payment events from anyone who finds
   the URL, in a field whose whole purpose is to be believed.
2. An `hmac` block naming `Stripe-Signature` whole is worse, because it *reads* as verification: it
   compares a digest against `t=…,v1=…`, which is not a digest, so it rejects every genuine delivery
   — and the day someone "fixes" the mismatch by relaxing the comparison, it accepts forgeries.

`stripe.webhook_signing_secret` is declared (`scheme = "signing"`, referenced by nothing) on Slack's
precedent, so the credential an operator must provision is named now rather than introduced later. It
is deliberately **not** a `[[config]]` field: asking for a value nothing can use teaches people to
paste secrets carelessly. It becomes one in the same change that adds the binding.

`stripe_connector.rs::stripe_declares_events_but_no_channel_binding_until_c141` is the tripwire — it
fails when a binding appears, so whoever adds one has to say why it is now expressible.

### Secret hygiene

No credential value anywhere. The one `[[config]]` field is the secret key, and it carries **no
`example`** — a Stripe key is the most heavily scanned secret there is, and this repository has
already lost a release to a `shpat_`-shaped placeholder tripping GitHub push protection. The
`sk_test` / `sk_live` distinction lives in `help`, where it cannot be copied as a value, and
`no_stripe_config_field_offers_a_credential_shaped_example` fails if a future edit puts more than four
base62 characters after `sk_live_`, `sk_test_`, `rk_live_` or `whsec_` in any renderable string.

## Notes
- Stripe's signature header packs two values into one header (`t=…,v1=…`), which `HmacSpec.prefix`
  may not express — it assumes a single literal prefix. **If it cannot be expressed, that is a
  finding worth more than the connector**: it means the matrix needs a parse rule, and it should be
  filed rather than worked around.
