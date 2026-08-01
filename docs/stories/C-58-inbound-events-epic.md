---
id: C-58
title: "Inbound events — connectors define the reverse call direction (epic)"
pillar: Spec
status: ready
priority: 1
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-spec, codegen, providers, bridge]
note: "EPIC — a connector today compiles only outbound ops; this adds the half where the vendor calls US. Verification is a declarable matrix (4 vendors, 1 parameterized HMAC), so it compiles rather than interprets — and flux's webhook channel has NO signature verification today, which is the blocking cross-repo seam"
---

# Inbound events — connectors define the reverse call direction (epic)

## Goal

Make a connector define **both directions**: the operations flux invokes, and the events the vendor
sends back. Today a connector is an API client — outbound ops only — so every connector-driven
automation must poll instead of reacting. This epic adds declared inbound events, generated signature
verification, generated subscription ops, and a polling fallback, without making this repo a runtime.

## Acceptance

- [ ] The provider TOML and IR carry an `[inbound]` section: transport, verification scheme,
      discriminator, delivery id, and per-event payload schemas (C-59).
- [ ] Verification is **generated from a declared scheme** and proven against real vendor signature
      vectors — GitHub, Stripe, Slack, Zendesk (C-60).
- [ ] Codegen emits event declarations, a manifest `[inbound]` block, and subscription ops (C-61, C-62).
- [ ] A `poll` transport presents the same event surface with no flux-side blocker (C-63).
- [ ] The flux-side verified-webhook seam is designed and its stories are filed on flux's board (C-64).
- [ ] Two vendors work end to end, inbound, against a live flux (C-65).
- [ ] Every invariant in the [design](../designs/inbound-events.md) has a named test — in particular
      **fail-closed**: a bad signature delivers *nothing* (assert a zero delivery count, not just an
      error response).

## Progress

- **"(not started)" was false and stood for months.** Corrected 2026-08-01 against the tree, not
  against a recollection. Measured this session:
  - **Five children are `done`:** C-60 (verification conformance), C-64 (the flux-side seam design),
    C-141, C-151, C-188.
  - **The inbound IR ships.** `crates/connector-spec/src/inbound.rs` declares **13 public types**
    (`grep -c "pub struct\|pub enum"` → 13): `Transport`, `FieldSource`, `Digest`, `Encoding`,
    `Selector`, `TimestampFormat`, `HmacSpec`, `VerificationScheme`, `EventDecl`, `Reply`,
    `Subscription`, `ManualSetup`, `ChannelBinding`.
  - **Three providers declare inbound surfaces** — `slack`, `stripe`, `twilio`
    (`grep -l "^\[\[events\]\]" providers/*.toml`) — and **8 events and 4 channels reach
    `web/public/catalog.json`**.
- **The epic's shape changed under it, which is why the log drifted.** Acceptance bullet 1 names an
  `[inbound]` section; what shipped is the `[[events]]` and `[[channels]]` **member kinds** under
  C-82's channel-bindings model. Same capability, different spelling — see C-59's own Progress.
- **Still genuinely open:** C-61 and C-62 (codegen — manifest/catalogue entries and subscription
  ops), C-63 (the `poll` transport), C-65 (two vendors end to end against a live flux). All four are
  `backlog`. The epic stays `ready` because of these, not because nothing has happened.
- **Read this before dispatching anyone at this epic.** Its Acceptance describes a design that was
  partly superseded; an implementor taking bullets 1–2 as unstarted work would rebuild what
  `inbound.rs` already holds. That is the `AGENTS.md` §Dispatching failure mode — acceptance must not
  assert a mechanism nobody has re-verified.

## Notes
- **The blocking fact, verified in flux at the time of writing:** `channel webhook` authenticates with
  an optional *static bearer token* and performs **no** signature verification
  (`crates/flux-channels/src/adapters/webhook.rs` has no HMAC path). A vendor that signs its payloads
  but cannot send a custom `Authorization` header has no authenticated route into flux at all.
- Sequenced like [C-16](C-16-design-auth-seam.md): the cross-repo seam (C-64) is designed **early** so
  flux can schedule it, while every other story proceeds without it.
- Non-goals restated because inbound is where they are most tempting to break: no relay, no daemon, no
  hosted endpoint, no unified cross-vendor event taxonomy, no event storage.
