---
id: C-144
title: "No connector can send a non-JSON request body"
pillar: Spec
status: ready
priority: 2
areas: [connector-spec, connector-flux]
note: "found shipping Stripe: op.rs binds application/json unconditionally and the IR has no content_type key anywhere. Blocks every form-encoded vendor — and OAuth2 token endpoints are form-encoded BY SPEC, so C-135's oauth2.login needs this too"
---

# No connector can send a non-JSON request body

## Goal

Let an operation declare how its body is encoded, so a connector can address a vendor that does not
accept JSON.

## What was measured

`crates/connector-flux/src/op.rs:144` and `:553` bind one media type unconditionally:

```rust
const JSON_MEDIA_TYPE: &str = "application/json";
```

and there is **no `content_type` / `encoding` / `body_format` key** on the provider, the service, the
operation or the `ParamSet`. So an operation declaring a body field always emits
`headers: { "content-type": "application/json" }` with a JSON document.

Stripe parses **only** `application/x-www-form-urlencoded`. So any Stripe operation with a body field
would send a document Stripe does not parse and get `400 Missing required param`.

## Why this is bigger than Stripe

C-106 worked around it with a selection rule — *an operation ships only if it addresses everything it
needs in the path* — which is why its refund is the legacy charge-nested
`POST /v1/charges/{charge}/refunds` rather than the canonical `POST /v1/refunds`, and why capture and
refund are full-amount only. That is a real fidelity loss, taken deliberately, on one connector.

It will recur on every form-encoded vendor: **Twilio, Mailgun, PayPal classic**, and others in the
fleet stories.

**And it blocks the authentication epic.** OAuth2 token endpoints are `application/x-www-form-urlencoded`
**by specification** (RFC 6749 §4.3.2 and friends). So
[C-135](C-135-authentication-role.md)'s `oauth2.login(grant: password, …)` cannot be emitted at all
until this lands. That dependency is not recorded in C-135 today and should be.

## Acceptance

- [ ] An operation can declare its request-body encoding. The set is **closed** — at minimum `json`
      and `form` — because an open string is a media type nobody validates, and a typo would ship a
      body the vendor silently ignores.
- [ ] `json` stays the default, so no shipped provider's emitted module changes. **A test asserts
      every existing module is byte-identical** across this story.
- [ ] The emitter encodes a `form` body as `application/x-www-form-urlencoded`, with the
      `content-type` header to match, and nesting refused rather than flattened — form encoding has
      no agreed nesting convention, and picking one silently is how a vendor receives a field it does
      not recognise.
- [ ] **Failing-first test:** `a_form_encoded_operation_emits_a_form_body_not_json` — must fail
      today, where the media type is a constant.
- [ ] Generated Flux still parses, analyzes and is a fixed point of flux's own formatter.
- [ ] `AGENTS.md`'s *Intentional gaps* list drops the entry this story closes.

## Notes

- **Coordinate with [C-135](C-135-authentication-role.md)**, which needs this. Whichever is scheduled
  first, C-135 should not start believing it can emit a token grant.
- Consider whether the encoding belongs on the operation or the service. A vendor is usually
  consistent, but Stripe's API is form-encoded while its *webhook* payloads are JSON — so the axis
  is per-request, not per-vendor, and the declaration should sit where that is expressible.
- `http.request`'s body argument is read with `Value::as_str`
  (`../flux/crates/flux-web/src/http.rs`), so the encoded form must reach it as **text**, the same
  way `parse($body, as: "json")` already canonicalises a record. Check what the equivalent is for a
  form body before assuming one exists — if flux has no form encoder, that is a flux-side story and
  the finding belongs on their board.
- Do not fix this by letting a provider write a raw `content-type` header. That would let a connector
  claim an encoding the emitter does not actually produce, which is worse than the current honest
  limitation.
