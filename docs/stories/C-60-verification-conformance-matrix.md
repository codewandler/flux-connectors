---
id: C-60
title: Verification conformance — one parameterized HMAC against real vendor vectors
pillar: Spec
status: ready
priority: 3
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-spec]
note: "the load-bearing test of the inbound half: 4 vendors' 'unique' schemes collapse to one algorithm over {digest, encoding, signed-template, tolerance} — proven with signature vectors from vendor docs, not self-generated fixtures"
---

# Verification conformance — one parameterized HMAC against real vendor vectors

## Goal

Prove that the declared verification matrix actually covers real vendors, so codegen can emit
verification instead of anyone hand-writing it per provider.

## Acceptance

- [ ] A conformance matrix test over GitHub, Stripe, Slack and Zendesk schemes, each with a
      **signature vector taken from the vendor's own documentation** — not a vector this repo generated
      with its own implementation.
- [ ] Failing-first test `vendor_signature_vectors_verify`, plus negative cases: wrong secret, mutated
      body, truncated signature, missing header, stale timestamp outside tolerance.
- [ ] Comparison is **constant-time**; a test or documented review note pins that (a `==` on the digest
      is the defect this exists to prevent).
- [ ] The signed-string template is applied over **raw bytes**; a test proves that re-serializing the
      JSON body (key reorder, whitespace) breaks verification, which is the behaviour we want.
- [ ] A vendor whose scheme does **not** fit the matrix produces an explicit "cannot verify"
      declaration, never a silent pass.

## Progress
- (not started)

## Notes
- Self-generated fixtures would agree with our own implementation and prove nothing — this repo has
  the same trap recorded elsewhere as guards tested against their own assumptions.
