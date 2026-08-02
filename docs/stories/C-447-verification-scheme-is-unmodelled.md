---
id: C-447
title: "The verification tri-state cannot say \"the vendor signs, and we cannot model how\""
pillar: Spec
status: ready
priority: 2
design: docs/designs/managed-agents-verification.md
epic: anthropic-managed-agents
areas: [connector-spec]
note: "found by C-446 against Anthropic Managed Agents, the third instance after C-141 and C-188 — but a different class. The other two were `HmacSpec` an axis short; this is `VerificationScheme` having no state for a signature we know exists and cannot yet describe, so the only declarations that load are a guess or a lie"
---

# The verification tri-state cannot say "the vendor signs, and we cannot model how"

## Goal

Give `ChannelBinding::verification` a fourth, **fail-closed** state meaning *the vendor signs this
delivery and this repository cannot yet express the scheme* — so a webhook whose signature is real but
unmodellable can be declared honestly instead of being declared falsely, guessed at, or withheld
entirely.

## What was measured

[C-446](C-446-managed-agents-events-and-verification.md) asked whether Anthropic's Managed Agents
webhook signature fits `HmacSpec`. It does not, and the blocking reason turned out not to be
`HmacSpec` at all.

The bundled `claude-api` reference (`shared/managed-agents-webhooks.md`, read 2026-08-02) states that
every delivery is HMAC-signed, names the three headers (`webhook-id`, `webhook-timestamp`,
`webhook-signature`) and the ~5-minute window — and **does not state the signed string, the digest,
the encoding, or the header-value format**, because it directs callers to the SDK's
`webhooks.unwrap()` instead. Five of `HmacSpec`'s nine fields are fillable; four are not.

`validate_channel_verification` (`crates/connector-spec/src/provider.rs:3047`, measured 2026-08-02)
admits three states, and **none of them is true here**:

| state | what it asserts | why it is wrong for this vendor |
|---|---|---|
| unset | — | a **loader error** on `webhook`, correctly |
| `verification = "none"` | *"the vendor publishes no signature"* | **false.** The vendor signs every delivery. A manifest saying otherwise tells a host there is nothing to check |
| `verification.hmac` | *"here are the parameters"* | four of them are unknown, and filling them from a convention is inventing a scheme |

The tri-state answers *"what does the vendor publish?"*. There is no state answering *"can this
repository express it?"* — two different questions collapsed into one field. The consequence is that
the only declarations that **load** today are a guess or a lie, and the only honest option left is to
ship no binding at all.

## Why this is worth a state rather than an omission

Shipping no binding is what Twilio did between C-109 and C-188 and what C-60 did for Stripe, and it
was right both times. It is also **lossy in a way those two were not**, and the difference is what
makes this a story:

- **The omission is indistinguishable from "this vendor has no inbound surface".** Nothing in the
  manifest, the catalogue or the published schema records that a signed webhook exists and was
  withheld. A reader six months later cannot tell a deliberate withholding from an oversight, which
  is the same failure mode `credential_response` (C-430) exists to prevent on the outbound side —
  there, an operation's exclusion is *named with its reason* rather than silently absent.
- **It is the wrong shape for a vendor whose events we do ship.** Managed Agents' webhook `data.type`
  vocabulary is a real, enumerable event set the connector should declare; only the *binding* is
  blocked. Withholding the binding leaves declared events no transport can carry.
- **`verification = "none"` will be reached for.** It loads, it silences the refusal, and its name
  reads like "no verification configured" rather than "the vendor publishes none". C-446 flagged it as
  the tempting wrong answer before any code was written; a state that is *correct* is a better defence
  than a rule that is merely documented.

## Acceptance

- [ ] **A fourth `VerificationScheme` state lands**, spelled so an author cannot mistake it for
      `"none"`, and carrying **why** the scheme is unmodellable as required prose — an empty marker
      would be `"none"` with extra steps. Record the spelling decision and its reason on the enum
      itself, the way `SIGNED_PLACEHOLDERS` records C-188's.
- [ ] **It is fail-closed, and a test proves it.** The state must mean *refuse the delivery*, never
      *accept unverified*. The hazard is precise: a host that reads an unrecognised verification state
      and falls through to "no check required" turns this story into the forgery hole C-141 closed.
      Name what a host must do, and assert the manifest carries it.
- [ ] **Failing-first test** in `crates/connector-spec/tests/verification_conformance.rs`, which is
      where C-60 put the real vendor vectors: a webhook binding declaring the new state loads, and
      the reference verifier returns a stated refusal for it rather than `Ok`.
- [ ] **`verification = "none"` is narrowed, not left as a synonym.** Once a truthful state exists,
      `"none"` means only what its doc comment already claims — *the vendor publishes no signature*.
      Decide whether the loader can tell the two apart (it probably cannot, since both are author
      claims) and, if not, say so in the refusal text and the doc comment rather than pretending.
- [ ] **It reaches the manifest, the catalogue and the published JSON schema.** C-141's Progress
      records `timestamp_format` landing in the IR and *not* in `ManifestHmac`/`HmacEntry`/`site.rs`,
      and the follow-up cost that carried. A verification state a host never sees is worse than none:
      it reads as safety while changing nothing (`AGENTS.md` § What flux actually keys on).
- [ ] The gate is green and the build stays a fixed point.

## Notes

- **Read [C-446](C-446-managed-agents-events-and-verification.md)'s design first** —
  [docs/designs/managed-agents-verification.md](../designs/managed-agents-verification.md) § *Two
  independent gaps*. It separates this gap (unconditional) from the `signed`-placeholder gap
  (conditional on a vendor fact nobody has read), and this story is **only** the first.
- **This is not C-141/C-188's class**, and conflating them will produce the wrong change. Those two
  widened `HmacSpec` because a vendor's *parameters* did not fit. This one is about a binding that has
  no parameters to fit yet. Do not open `SIGNED_PLACEHOLDERS`.
- **The other half of the Managed Agents gap is not filed and should not be.** Whether `webhook-id`
  enters the signed string is unverified; filing a story against an unverified premise is the dispatch
  failure `AGENTS.md` records against C-413. It becomes a story when a vendor document says so.
- This changes verification IR that `connector-pack` reads, so — like C-188 — **it runs solo**.
- Do not edit `providers/anthropic.toml` as part of this story; the Managed Agents provider file is
  its own work and is contended (C-441).

## Progress
- (not started)
