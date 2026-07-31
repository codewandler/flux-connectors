---
id: C-188
title: "HmacSpec can sign a body and a timestamp and nothing else, so a form-posting webhook vendor cannot be verified"
pillar: Spec
status: ready
priority: 2
epic: inbound-events
areas: [connector-spec, connector-pack]
note: "found by C-109: Twilio signs the request URL plus its sorted, reassembled form fields. HmacSpec::signed admits only {body} and {timestamp}, so Twilio ships its events with NO channel binding — the honest outcome, and the second instance of this class after C-141's composite header"
---

# `HmacSpec` can sign a body and a timestamp and nothing else, so a form-posting webhook vendor cannot be verified

## Goal

Let a connector express a webhook signature computed over something other than the raw body, so a vendor
whose scheme includes the URL or reassembled form fields can be verified rather than left unverified.

## What was measured

[C-109](C-109-provider-twilio.md) shipped Twilio's read surface and **could not ship its webhook binding.**
Twilio's `X-Twilio-Signature` is an HMAC over **the full request URL concatenated with the POST body's
form fields, sorted by key and reassembled**. `HmacSpec::signed` admits `{body}` and `{timestamp}` and
nothing else.

So Twilio declares its `[[events]]` with **no `[[channels]]` binding at all**, and a test asserts that
absence. That is the correct outcome — the member contract's *"silence is never a verification answer"*
rule governs a binding that exists, and declaring one whose verification cannot be performed would be
worse than declaring none.

## This is the second instance, which is what makes it a story

[C-141](C-141-hmac-signed-template.md) found the first: Stripe's composite `Stripe-Signature` header packs
a timestamp and one or more signatures into one value, and `signed = "{timestamp}"` **loaded cleanly**
while signing a body-independent string — one captured signature would have verified any forged payload.

The pattern across both: **`HmacSpec` models "sign these bytes" when real vendors sign a *derivation* of
the request.** Named vendors that will hit it:

| vendor | signs over |
|---|---|
| Twilio (C-109) | request URL + form fields sorted and reassembled |
| Mailgun | timestamp + token, concatenated — not the body at all |
| PayPal classic (IPN) | a round-trip *back* to the vendor, not an HMAC |
| Shopify | raw body (already works) |

## Acceptance

- [ ] A connector can express a signature over the request **URL**, and over **form fields reassembled in
      a defined order**. Decide whether that is more template placeholders (`{url}`, `{sorted_form}`) or a
      named closed set of schemes (`twilio_v1`, `stripe_v1`, …), and **record the decision with its
      reason**. A template is more expressive and lets an author write a derivation the verifier does not
      actually compute; a closed set refuses the unknown vendor. Both are defensible; say which and why.
- [ ] **Whatever lands must make C-141's failure impossible, not merely unlikely.** A spec that signs a
      body-independent string must not load. That is the load-bearing test: `signed = "{timestamp}"` alone
      was accepted once and one captured signature would have verified any forged payload.
- [ ] **Failing-first test:** Twilio's scheme cannot be declared today.
- [ ] `providers/twilio.toml` gains its `[[channels]]` binding, or this story records why it still cannot —
      it is the concrete case and the natural proof.
- [ ] Every existing verification is byte-identical, and the shipped inbound surfaces (18 header
      placements, 2 inbound per C-159's count) are unchanged.

## Notes

- Read C-141's Progress first. It is the same class and it already established that **an unchecked
  arithmetic path in a verification spec is a security defect, not a validation nicety** — its
  `parse_tolerance` multiplied unchecked, so in release `i64::MAX * 60` wrapped to `Ok(-60)`, a negative
  window that loaded.
- **A verification a connector cannot perform must stay undeclarable.** The one thing worse than Twilio
  having no binding is Twilio having a binding that reports success without verifying anything.
- PayPal classic is deliberately in the table above and out of scope: it is not an HMAC at all but a
  callback to the vendor, so it wants a different mechanism and should not be forced into this one.
- This runs solo — it changes verification IR that `connector-pack` reads.
