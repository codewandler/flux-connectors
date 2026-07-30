---
id: C-141
title: "Four gaps C-60 found in HmacSpec, one of which is a forgery hole by construction"
pillar: Spec
status: in-progress
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

- [x] **A `signed` template that never interpolates `{body}` is refused.** `signed = "{timestamp}"`
      with a selector and a tolerance loads cleanly today and signs a body-independent string — so one
      captured signature verifies **any** forged payload for the whole window. This is the same class
      of defect as the unterminated-brace bug C-60 fixed, but reachable with no typo at all.
      `validate_hmac` already refuses an *empty* placeholder set; it must also refuse a set missing
      `body`. **Failing-first test required**, and it must demonstrate the forgery, not just the
      refusal.
- [x] **`tolerance` is parsed.** The loader requires one on a timestamped scheme but has no opinion on
      its shape, so `tolerance = "banana"` loads and the replay window becomes whatever a host decides
      at runtime. Add `parse_tolerance` in `inbound.rs`, called from `validate_hmac`. C-60's
      `every_shipped_tolerance_is_a_window_a_host_can_actually_apply` is the stopgap and should become
      redundant.
- [x] **A body-sourced verification timestamp is refused.** `HmacSpec::timestamp` is a full `Selector`
      today, so a connector can declare a timestamp read from the body — which requires parsing
      *before* verifying, inverting the order that makes verification meaningful. flux's own C-291
      refuses it; the loader should refuse it first, so the failure lands in a build rather than in an
      operator's runtime.
- [x] **A timestamp *format* axis.** `HmacSpec` says where the timestamp is read from and never how it
      is spelled: Slack and Stripe send unix seconds, Zendesk sends RFC 3339. C-60's reference verifier
      has to sniff — which is exactly the guessing the `timestamp` selector was added to stop.
- [x] The gate is green; the build stays a fixed point.

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

## Progress

All four gaps are closed in the loader; the gate is green and
`cargo run -p connector-cli -- build` reports `19 providers, 256 artifacts up to date; nothing
written`, so no shipped provider's emitted output moved. Only `providers/slack.toml` ships an HMAC
binding, it sends unix seconds, and it needed no edit.

- **The forgery is demonstrated, not just refused.**
  `verification_conformance.rs::a_signed_template_that_omits_the_body_verifies_a_forged_payload` takes
  the *shipped* Slack spec, deletes `{body}` from `signed` and nothing else, captures a signature over
  one body and shows `verify` accepting a different one — then demands the loader refusal. The first
  half keeps running after the fix, because it is the reason for it. At the merge base the test reached
  step 4 and failed there: the forgery verified and the declaration loaded.
- **`inbound::parse_tolerance`** is the crate's one duration parser, called from `validate_hmac`. It
  refuses no-unit, non-integer, zero, and anything over `MAX_TOLERANCE_SECONDS` (1h). C-60's
  `every_shipped_tolerance_is_a_window_a_host_can_actually_apply` became structurally redundant — every
  spec `shipped_specs()` returns has already been through `provider::load` — and was rewritten as
  `a_tolerance_no_host_could_apply_does_not_load`, which tests the loader's refusal directly and keeps
  the old sweep as an assertion that it is redundant. The conformance verifier's private copy of
  `parse_tolerance` is gone; it imports the crate's.
- **`TimestampFormat`** (`unix_seconds` | `rfc3339`) is a new `HmacSpec` field,
  `Option<TimestampFormat>`, absent meaning `unix_seconds`. It could not be made mandatory:
  `providers/slack.toml` is fenced for this story, and requiring the field would stop it loading. The
  reference verifier no longer sniffs — `parse_timestamp` takes the format as a parameter, and
  `the_declared_timestamp_format_is_read_instead_of_sniffed` pins the hazard it removes
  (`20220505183228` is a valid integer and a plausible date; sniffing dated it to the year 642,000, so
  no window could call it stale).
- **What did not follow the field downstream.** `timestamp_format` reaches the IR, the loader and the
  published JSON schema, but *not* `connector-cli`'s manifest, seam and site projections — those files
  belong to other stories in this wave, and adding a field there rewrites committed artifacts, which a
  scoped run must not do. Nothing shipped declares a non-default format, so nothing is currently lost;
  a follow-up must carry it into `ManifestHmac`/`HmacEntry` and flux's `verify` block **before** the
  first RFC 3339 vendor binding ships, or that connector's host will read the wrong spelling.
- **Stripe stays out**, as the story directs. `stripe_declares_events_but_no_channel_binding_until_c141`
  still passes with `connector.channels` empty. The composite `Stripe-Signature` extraction axis is
  untouched, and that test's name and prose now point at a story that closed without it — it needs
  repointing at whichever story takes the extraction axis.
