---
id: C-59
title: An `[inbound]` section in the provider TOML and the IR
pillar: Spec
status: ready
priority: 2
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-spec]
note: "pure functions from bytes to IR, no network: transport, verification, discriminator, delivery id, per-event `when` narrowing and payload schema refs"
---

# An `[inbound]` section in the provider TOML and the IR

## Goal

Extend the spec front-end so a provider can declare what it sends us, with the same discipline the
outbound side already has: typed, hermetic, and provenance-tracked.

## Acceptance

- [ ] `[inbound]` parses into a new IR module: `transport`, `verification`, `discriminator`,
      `delivery_id`, and `[[inbound.event]]` entries with `name`, optional `when`, optional `schema`.
- [ ] Failing-first test `inbound_toml_round_trips_to_ir` over a fixture covering all four verification
      shapes in the design's table.
- [ ] `secret`/`secret_ref` in a verification block is a **credential name**; a literal-looking value is
      a **parse error**, not a warning (invariant 3 — the secret must never be able to enter an artifact).
- [ ] A `{timestamp}` in the `signed` template without a `tolerance` is a parse error (invariant 5).
- [ ] Event payload schema refs resolve against the vendored spec cache, and an unresolvable ref fails
      the build rather than degrading to untyped.
- [ ] Inbound facts participate in provenance and the lockfile, so drift detection covers them.

## Progress
- (not started)
