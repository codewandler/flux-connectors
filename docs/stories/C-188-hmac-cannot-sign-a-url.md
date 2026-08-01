---
id: C-188
title: "HmacSpec can sign a body and a timestamp and nothing else, so a form-posting webhook vendor cannot be verified"
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/inbound-events.md
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

- [x] A connector can express a signature over the request **URL**, and over **form fields reassembled in
      a defined order**. Decide whether that is more template placeholders (`{url}`, `{sorted_form}`) or a
      named closed set of schemes (`twilio_v1`, `stripe_v1`, …), and **record the decision with its
      reason**. A template is more expressive and lets an author write a derivation the verifier does not
      actually compute; a closed set refuses the unknown vendor. Both are defensible; say which and why.
      → **placeholders**, with the three reasons recorded on `SIGNED_PLACEHOLDERS`
      (`crates/connector-spec/src/inbound.rs`).
- [x] **Whatever lands must make C-141's failure impossible, not merely unlikely.** A spec that signs a
      body-independent string must not load. That is the load-bearing test: `signed = "{timestamp}"` alone
      was accepted once and one captured signature would have verified any forged payload.
      → the rule is now `PAYLOAD_PLACEHOLDERS`, not the literal `{body}` (`provider.rs`'s
      `validate_hmac`), and
      `verification_conformance.rs::a_signed_template_that_covers_only_the_url_verifies_a_forged_payload`
      demonstrates the forgery the widened vocabulary would otherwise have admitted.
- [x] **Failing-first test:** Twilio's scheme cannot be declared today.
      → `channel_bindings.rs::twilios_url_and_sorted_form_scheme_is_declarable`.
- [x] `providers/twilio.toml` gains its `[[channels]]` binding, or this story records why it still cannot —
      it is the concrete case and the natural proof.
      → two bindings, one per callback URL, both verified; and Twilio's **own published signature**
      reproduces through them in `verification_conformance.rs::vendor_signature_vectors_verify`.
- [x] Every existing verification is byte-identical, and the shipped inbound surfaces (18 header
      placements, 2 inbound per C-159's count) are unchanged.
      → **with one intended correction, stated rather than ticked past.** Slack's binding, its
      parameters and its artifacts are untouched; the only artifacts the build moved are Twilio's three
      (`connectors/twilio.connector.toml`, `connectors.lock`, `web/public/catalog.json`), and
      `connector-cli -- diff` still reports **937 artifacts up to date**. The inbound *surface* is not
      unchanged and could not be: it grew from **2 channel bindings to 4**, which is this story's goal.
      No auth placement moved.

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

## Progress

**Twilio can now be verified end to end, and the proof is the vendor's own published signature.**
`verification_conformance.rs`'s Twilio row is `Source::VendorDocumented`: it reads the shipped
`providers/twilio.toml` binding through the real loader, feeds the reference verifier Twilio's
documented URL, form body and auth token, and reproduces `L/OH5YylLD5NRKLltdqwSvS0BnU=`. The verifier
still contains no `match vendor`, which is what makes that a conformance result rather than a fifth
implementation.

### The decision: placeholders, not named schemes

`SIGNED_PLACEHOLDERS` grew from `["body", "timestamp"]` to `["body", "sorted_form", "timestamp",
"url"]`. The reasoning is written where a future author meets it, on the constant itself; in short:

1. A closed set of named schemes (`twilio_v1`) **is** the per-vendor branch
   `verification_conformance.rs`'s first rule forbids, spelled as data — and it moves the vendor's
   parameters out of the connector, where a drift check cannot see them.
2. The placeholder list **already is** the closed set. The objection to a template — that an author can
   write a derivation the verifier does not compute — is answered by the loader refusing the name. The
   template has no operators, no repetition and no ordering primitive; the only thing it composes is
   literal text, which is where vendors genuinely differ (Slack's `v0:`, Stripe's `.`, Twilio's
   nothing-at-all).
3. Refusing the unknown vendor sounds like the safe default and is not: a vendor whose axes already fit
   this vocabulary would be refused for having no *name*, and its honest workaround is the unverified
   endpoint this story exists to close.

### `{body}` was never the rule; `PAYLOAD_PLACEHOLDERS` is

The part that mattered most, and the reason widening the vocabulary was not a one-line change. `{url}`
is a **per-endpoint constant** — every delivery to one callback URL signs the same string — so
`signed = "{url}"` is exactly C-141's `signed = "{timestamp}"` with a longer constant, and *worse*,
because a URL-signing vendor carries no timestamp and therefore no `tolerance` bounding the replay at
all. It would have loaded the moment `{url}` became fillable, under a rule that still read as though it
said what it used to say. The loader now tests membership of `PAYLOAD_PLACEHOLDERS` (`{body}`,
`{sorted_form}`), and `a_signed_template_that_covers_only_the_url_verifies_a_forged_payload`
demonstrates the forgery on Twilio's *shipped* parameters before demanding the refusal — the same shape
C-141's test takes, for the same reason.

### The form-encoder gap does not touch this

Checked before building, because a declaration that cannot be performed would be worse than the honest
gap. `BodyEncoding::Form` refuses a **nested** body because vendors disagree about how to spell nesting
(`metadata[key]`, `a[b]`, `a[b][]`), and the encoder that would settle it is upstream flux work
(`L-101`). That is the *outbound* direction: producing a form body from a nested structure. A callback
arrives as a flat sequence of `name=value` pairs and `{sorted_form}` only reads it — no nesting is
reachable, so no convention has to be chosen. The two gaps run in opposite directions and do not touch.

What the derivation *did* need pinning down, each step forced by Twilio's own vector: percent-decode
(the example signs `To+18005551212` where the wire carries `To=%2B18005551212`, so a raw splice
reproduces nothing), sort byte-wise by the decoded name (UTF-8 order and code-point order agree, so no
correct implementation can diverge), join name-onto-value with no delimiter anywhere.

**A repeated field name is refused, not resolved.** `a=1&a=2` has no defined answer — Twilio's own
helper libraries build a map, so which value wins depends on the language — and two correct
implementations disagreeing on an authentication path means one accepts what the other rejects. The
reference verifier returns a stated `CannotVerify`. This is a *runtime* rule and not a load rule,
because a forger picks the body; the loader cannot see it.

### What else moved, and what did not

- **HMAC-SHA1 is implemented rather than refused.** The reference verifier returned
  `CannotVerify("sha1 signatures are not verified by this matrix")`, which for a shipped Twilio binding
  would have meant declaring a scheme this repository never reproduced — the exact outcome the story
  forbids. SHA-1 is written out in the test file (no dependency added; manifests are fenced and the
  workspace pins no SHA-1 crate) and pinned to **RFC 2202 §3** cases 1, 2 and 6, so Twilio's row is
  accountable to something outside this repository.
- **Two Twilio bindings, not one.** A status callback carries no field naming its own type — a message
  callback is known by carrying `MessageSid` — and `discriminator` addresses one field, not a
  disjunction. Not a gap: Twilio configures callbacks **per resource**, so the messaging and voice
  callback URLs are separate URLs an operator wires separately. One binding per URL, one event each,
  nothing left to discriminate.
- **No `delivery_id` on either binding, deliberately.** Twilio sends no per-delivery identifier, and the
  tempting stand-in is wrong: `MessageSid` identifies the *message*, so deduping on it would collapse
  `queued` → `sent` → `delivered` into one event. Recorded in the provider file rather than left blank.
- **`twilio.webhook_signing_secret` is finally referenced.** Declared and unused since C-109, which
  `twilio_connector.rs` used to assert.
- **The published JSON schema and `site.rs`'s `signed` doc were restated**, both of which named the old
  two-placeholder vocabulary. C-141's Progress records how that schema drifted last time, so it was
  corrected in the same change rather than left to a follow-up.

### What a follow-up must not miss

`signed = "{url}{sorted_form}"` reaches flux verbatim through the manifest, and **flux's `verify` block
must learn both derivations before this connector's host can act on it** — the seam
(`docs/designs/verified-webhook-seam.md`, flux's C-291…C-295) predates the vocabulary. `{url}` carries a
deployment hazard the seam has to state: behind a proxy the host sees a rewritten scheme, authority or
path, while the signature covers the URL the vendor was configured with, so a host that signs what it
received rejects every genuine delivery. Both bindings' `[channels.setup]` steps say "exactly as shown"
for that reason — which is documentation, not a guarantee.
