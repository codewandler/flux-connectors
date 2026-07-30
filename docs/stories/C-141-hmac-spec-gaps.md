---
id: C-141
title: "Four gaps C-60 found in HmacSpec, one of which is a forgery hole by construction"
pillar: Spec
status: ready
priority: 2
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-spec]
note: "found by C-60's conformance work, measured not guessed. A `signed` template that never interpolates {body} loads cleanly and signs a body-independent string — the same class as the brace typo C-60 fixed, but reachable without any typo"
---

# Four gaps C-60 found in HmacSpec, one of which is a forgery hole by construction

## Goal

Close the four declaration gaps [C-60](C-60-verification-conformance-matrix.md) surfaced while
building the conformance matrix. It found and fixed one forgery hole; these are the ones it could not
reach because they need `provider.rs`, which was fenced.

## Acceptance

- [ ] **A `signed` template that never interpolates `{body}` is refused.** `signed = "{timestamp}"`
      with a selector and a tolerance loads cleanly today and signs a body-independent string — so one
      captured signature verifies **any** forged payload for the whole window. This is the same class
      of defect as the unterminated-brace bug C-60 fixed, but reachable with no typo at all.
      `validate_hmac` already refuses an *empty* placeholder set; it must also refuse a set missing
      `body`. **Failing-first test required**, and it must demonstrate the forgery, not just the
      refusal.
- [ ] **`tolerance` is parsed.** The loader requires one on a timestamped scheme but has no opinion on
      its shape, so `tolerance = "banana"` loads and the replay window becomes whatever a host decides
      at runtime. Add `parse_tolerance` in `inbound.rs`, called from `validate_hmac`. C-60's
      `every_shipped_tolerance_is_a_window_a_host_can_actually_apply` is the stopgap and should become
      redundant.
- [ ] **A body-sourced verification timestamp is refused.** `HmacSpec::timestamp` is a full `Selector`
      today, so a connector can declare a timestamp read from the body — which requires parsing
      *before* verifying, inverting the order that makes verification meaningful. flux's own C-291
      refuses it; the loader should refuse it first, so the failure lands in a build rather than in an
      operator's runtime.
- [ ] **A timestamp *format* axis.** `HmacSpec` says where the timestamp is read from and never how it
      is spelled: Slack and Stripe send unix seconds, Zendesk sends RFC 3339. C-60's reference verifier
      has to sniff — which is exactly the guessing the `timestamp` selector was added to stop.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **Every one of these was measured by C-60, not predicted.** Read its story and
  `crates/connector-spec/tests/verification_conformance.rs` before starting — the matrix will tell
  you immediately whether a change breaks a real vendor scheme.
- Stripe's composite `Stripe-Signature` header (`t=…,v1=…`) is a **separate** and larger gap:
  `HmacSpec` has one literal `prefix` and a `Selector` addressing a whole header, so no component can
  be taken out of that list. That needs a new extraction axis and is its own story, not this one.
  C-60 declares Stripe `cannot verify` rather than pretending, which is the right interim state.
- The first bullet is the one to do first. The others are correctness and ergonomics; that one is a
  signature scheme that verifies forgeries.
