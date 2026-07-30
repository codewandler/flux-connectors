---
id: C-62
title: Codegen — webhook subscription ops from the vendor spec
pillar: Codegen
status: backlog
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-flux, providers]
note: "registering a webhook is an ordinary outbound op, so it needs no new machinery — and it correctly inherits the authorization → approval → guarded-IO envelope instead of being a build-time side effect"
---

# Codegen — webhook subscription ops from the vendor spec

## Goal

Emit the operations that register, list and remove a vendor's webhook subscription, so the inbound half
can be set up and torn down from a flow rather than by hand in a vendor console.

## Acceptance

- [ ] `<name>_webhook_subscribe` / `_unsubscribe` / `_list` generated from the vendor spec, selected via
      the provider TOML like any other operation.
- [ ] Failing-first test `subscription_ops_are_generated_and_typed` for a vendor whose spec publishes
      the endpoints (GitHub) and a documented outcome for one whose spec does not.
- [ ] These are **ordinary ops** — no special-casing, and they traverse the same authorization and
      approval path as any other write (invariant 6).
- [ ] The generated secret parameter is an auth reference, so the subscription call cannot leak the
      signing secret into a transcript or artifact.

## Progress
- (not started)

## Notes
- Endpoint lifecycle ownership (operator-run setup flow vs. a program reconciling at startup) is an
  open question in the design; a startup reconciler risks duplicate webhooks on a restart loop.
