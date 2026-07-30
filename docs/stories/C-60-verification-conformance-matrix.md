---
id: C-60
title: Verification conformance — one parameterized HMAC against real vendor vectors
pillar: Spec
status: in-progress
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
      with its own implementation. *GitHub and Slack carry the vendors' own published triples; Zendesk
      and Stripe publish parameters but no worked triple, so their provenance is stated per row rather
      than overclaimed — see Progress.*
- [x] Failing-first test `vendor_signature_vectors_verify`, plus negative cases: wrong secret, mutated
      body, truncated signature, missing header, stale timestamp outside tolerance.
- [x] Comparison is **constant-time**; a test or documented review note pins that (a `==` on the digest
      is the defect this exists to prevent).
- [x] The signed-string template is applied over **raw bytes**; a test proves that re-serializing the
      JSON body (key reorder, whitespace) breaks verification, which is the behaviour we want.
- [x] A vendor whose scheme does **not** fit the matrix produces an explicit "cannot verify"
      declaration, never a silent pass.

## Progress

`crates/connector-spec/tests/verification_conformance.rs` is the matrix: one reference verifier
parameterized **only** by `HmacSpec`, with no vendor branch in it, run over four rows whose
parameters all come through the real loader.

**What the vectors actually prove, per row.** The distinction the story insists on is recorded in
each row's `Source` rather than flattened:

| vendor | provenance | outcome |
|---|---|---|
| GitHub | vendor-published `(secret, body, signature)` triple | verifies |
| Slack | vendor-published triple, checked against the **shipped** `providers/slack.toml` binding | verifies |
| Zendesk | vendor-published *parameters*; no worked triple exists, so the digest comes from CPython's `hmac` — an implementation outside this repository, not this one agreeing with itself | verifies |
| Stripe | vendor-published *header*, used to show the scheme does **not** fit | stated `cannot verify` |

The HMAC primitive is pinned separately to **RFC 4231**'s published vectors. That is what makes the
vendor rows evidence rather than a tautology: an implementation that reproduces RFC 4231 *and*
reproduces GitHub's and Slack's documented digests from their documented inputs did not get there by
agreeing with itself.

**Stripe does not fit, and that is the finding.** Its signed string (`{timestamp}.{body}`) is
expressible; its **header** is not. `Stripe-Signature` is a comma-separated key/value list carrying
the timestamp and one digest per scheme version, while `HmacSpec` has a single literal `prefix` and a
`Selector` that addresses a whole header — neither can take a component out of that list. Closing it
needs a new extraction axis on `HmacSpec` plus loader support, which is a story of its own.

**The failing-first defect was real, and it was in `signed_placeholders`.** An unterminated `{` was
silently swallowed, so `signed = "v0:{timestamp}:{body"` — one missing brace — passed every loader
check (non-empty placeholder list, all names fillable, selector and tolerance present) and produced a
signed string that does not contain the body at all. A signature captured from one delivery then
verifies **any** forged payload. The fragment is now reported verbatim as a placeholder no host can
fill, so the loader's existing refusal catches it; no loader change was needed.

**Two gaps this test had to work around, both worth their own stories.** `HmacSpec` says *where* a
timestamp is read from but never *how it is spelled* (Slack and Stripe send unix seconds, Zendesk
sends RFC 3339), so the reference verifier has to sniff — which is exactly the guessing the
`timestamp` selector was added to stop. And nothing parses `tolerance`, so `tolerance = "banana"`
loads today; `every_shipped_tolerance_is_a_window_a_host_can_actually_apply` is the stopgap gate.

## Notes
- Self-generated fixtures would agree with our own implementation and prove nothing — this repo has
  the same trap recorded elsewhere as guards tested against their own assumptions.
