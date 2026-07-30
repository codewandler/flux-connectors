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
- (not started)

## Notes
- **The blocking fact, verified in flux at the time of writing:** `channel webhook` authenticates with
  an optional *static bearer token* and performs **no** signature verification
  (`crates/flux-channels/src/adapters/webhook.rs` has no HMAC path). A vendor that signs its payloads
  but cannot send a custom `Authorization` header has no authenticated route into flux at all.
- Sequenced like [C-16](C-16-design-auth-seam.md): the cross-repo seam (C-64) is designed **early** so
  flux can schedule it, while every other story proceeds without it.
- Non-goals restated because inbound is where they are most tempting to break: no relay, no daemon, no
  hosted endpoint, no unified cross-vendor event taxonomy, no event storage.
