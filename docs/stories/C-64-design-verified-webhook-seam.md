---
id: C-64
title: Design the flux-side verified-webhook seam and file its flux stories
pillar: Bridge
status: done
design: docs/designs/verified-webhook-seam.md
epic: inbound-events
areas: [bridge]
note: "the C-16 pattern repeated: flux's webhook channel has NO signature verification (bearer token only), so generated verification has nowhere to run — designed in verified-webhook-seam.md and filed on flux's board as C-291…C-295, so every other inbound story proceeds without it"
---

# Design the flux-side verified-webhook seam and file its flux stories

## Goal

Specify what flux must gain for a verified, typed inbound event to be possible at all, and hand those
stories to flux's board early — this ships in a different repository on a different cadence, so it
blocks the finish, not the start.

## Acceptance

- [x] A design section (in [inbound-events.md](../designs/inbound-events.md)) specifying the six
      flux-side capabilities: a declarative `verify` block on `channel webhook`; verification over the
      **raw body before parsing**; constant-time comparison plus timestamp tolerance; discriminator →
      trigger-label routing; a challenge/handshake hook answered without waking an agent; and the
      delivery id in the payload.
      → the section stands at [inbound-events.md](../designs/inbound-events.md) §"The flux-side seam"
      and now points at [verified-webhook-seam.md](../designs/verified-webhook-seam.md), which designs
      all six in full: §1 request path, §2 how `HmacSpec` reaches it, §3 failure behaviour, §4 secret
      supply, §5 the six capabilities mapped to their filed stories.
- [x] A handoff artifact — [inbound-events-flux-stories.md](../designs/inbound-events-flux-stories.md)
      — carrying ready-to-paste flux stories, explicitly marked as **not** this board's backlog so
      `/track:board` never picks them up.
      → the drafts are no longer drafts: it is now the **filing record**, with the F-n → C-29n map,
      the flux paths, and the load-bearing facts re-verified. The not-this-board's-backlog banner is
      unchanged.
- [x] Story ids in the handoff are marked **provisional** with the re-check command, because flux's
      fleet allocates ids concurrently (the C-16 handoff's claimed range was consumed by unrelated work
      before it was pasted — do not repeat that assumption).
      → the lesson is kept and applied rather than merely restated: the ids were checked free
      immediately before each file was written, and re-checked after, against a concurrent filer. The
      re-check command is still in the document for the next handoff.
- [x] Every flux-side claim is anchored to a symbol, not a line number, and states the flux version it
      was verified against.
      → every citation was read at flux `v0.40.0-4-g2abd0a13` (workspace version `0.40.0`), stated in
      the design's provenance block and in each filed story's context section, with the
      re-grep-by-symbol instruction.

## Progress

- **Done.** Designed and filed.
- Design: [`docs/designs/verified-webhook-seam.md`](../designs/verified-webhook-seam.md) — new, and the
  story's `design:` now points at it rather than at the parent.
- Filed on flux's board (**uncommitted** in flux's working tree; flux's `/track:board` is not run by
  us, and flux's `docs/stories/README.md` was not touched):
  - `C-291` — `../flux/docs/stories/C-291-webhook-verify-raw-body.md`
  - `C-292` — `../flux/docs/stories/C-292-webhook-signature-schemes.md`
  - `C-293` — `../flux/docs/stories/C-293-webhook-challenge-handshake.md`
  - `C-294` — `../flux/docs/stories/C-294-webhook-discriminator-routing.md`
  - `C-295` — `../flux/docs/stories/C-295-delivery-envelope-verified-flag.md`
  All five: `pillar: Core`, `status: backlog`, `epic: verified-webhook-channel`, no `design:` field
  (flux has no design doc for this seam; each story's Notes cites this repository's path instead).
- No Rust was written or touched. `crates/connector-spec/src/inbound.rs` was **read only** — C-60 owns
  it.

## Two findings for the connector side — these need owners, and it is not this story

Writing the flux side surfaced two things the connector-side IR gets wrong or cannot express. Both are
recorded in [verified-webhook-seam.md](../designs/verified-webhook-seam.md) §2 with evidence; neither
was changed here.

1. **`HmacSpec::timestamp` is wider than a verify-before-parse host can honour.** `Selector`
   (`crates/connector-spec/src/inbound.rs:100-105`) admits `FieldSource::Body` (`:74`), and
   `HmacSpec::timestamp` is an `Option<Selector>` (`:151`). A body-sourced timestamp is
   **unimplementable by construction**: it is an input to the comparison that decides whether the body
   may be parsed at all. flux will refuse it at load, but the refusal belongs at *build* time in the
   repository that owns the declaration. **Owner: C-59/C-60** — a loader rule that `HmacSpec::timestamp`
   must be `FieldSource::Header`. `discriminator` and `delivery_id` are unaffected: both are read after
   the decode, so `FieldSource::Body` is legitimate for them.

2. **Stripe's composite header is not expressible.** `Stripe-Signature: t=…,v1=…,v0=…` — the digest is
   neither the whole header value nor a literal prefix of it, so `prefix: Option<String>` (`:137`)
   cannot select it; and the timestamp is a *component of that same header*, which
   `Selector { source: Header, name }` cannot address. The doc comment at `:145` already says "Stripe's
   `t=` component", so the intent is recorded and the grammar is not there. Stripe also sends **several
   `v1=` during a secret rotation**, so a verifier must accept if any candidate matches. **Owner:
   C-59/C-60**; three options are laid out in the design and none is chosen here. flux's side is
   unaffected in shape — C-292 requires a *set* of candidate digests either way — so the flux stories
   are not blocked, but the conformance matrix (C-60) is.

## Notes

- Verified in flux at `v0.40.0-4-g2abd0a13`: `crates/flux-channels/src/adapters/webhook.rs` has an
  optional static bearer token and **no** HMAC/signature path. `WebhookSettings` is
  `{ addr, path, async, token }` (`crates/flux-channels/src/config.rs:18-32`).
- **The structural finding:** `Json(body): Json<Value>` (`webhook.rs:86`) is an axum *extractor*, so
  the body is deserialized before the handler runs. "Verify the raw body before parsing" is therefore
  not a line that can be inserted — changing the handler signature to `Bytes` is the change, and
  everything else layers on it. A side effect: the existing bearer check already runs post-parse.
- **The secret path needs no new machinery**, which is the happiest part of the design.
  `secret: secret "ENV"` inside a nested record parses today (`crates/flux-lang/src/cst_decode.rs:2127-2130`,
  reached at any depth), `resolve_secrets` registers it with the shared `Redactor` before channels are
  built (`crates/flux-app/src/secrets.rs:43`), and `build_channels` refuses any surviving marker
  (`crates/flux-channels/src/adapters/mod.rs:39-45`). Three caveats with required responses are in §4:
  the redactor's silent 6-character floor, `WebhookSettings`' derived `Debug`, and the two paths in the
  adapter that go around the redactor.
- **No new third-party dependency for flux:** `hmac`, `sha2`, `base64` and `hex` are already workspace
  dependencies (`Cargo.toml:150-153`), flux-providers already computes HMAC-SHA256
  (`crates/flux-providers/src/bedrock.rs:32,42`), `constant_time_eq` already exists in the webhook
  adapter (`webhook.rs:123-132`), and flux-channels is L6 (`crates/flux-codegate/src/lib.rs:53-54`).
- Naming caution, exactly as C-16 hit: flux already has a **done** inbound `request-auth-seam`
  (bearer → principal). Call this one **webhook signature verification**, never "the inbound auth seam".
  Every filed story repeats this in its Notes.
- **Corrected a stale claim inherited from the previous handoff:** `AppDeliverer` does *not* serialize
  deliveries behind a mutex. It is `{ app: Arc<App> }` and forwards
  (`crates/flux-channels/src/deliver.rs:22-39`); admission is a semaphore with
  `DEFAULT_MAX_INFLIGHT_DELIVERIES = 64` (`crates/flux-app/src/admission.rs:49`). Up to 64 verified
  deliveries run concurrently, so nothing in this seam may assume delivery serialization.
