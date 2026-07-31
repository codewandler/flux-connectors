---
id: C-212
title: "The host's `connected` repeats C-206's conflation in the surface an operator actually looks at"
pillar: Bridge
status: ready
priority: 2
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, host]
note: "found by the C-206 implementor 2026-07-31 and verified at api.rs:168. C-206 fixed the conflation in the published catalogue; the host reproduces it one layer up, as a single boolean, in the view the operator wires connectors from"
---

# The host's `connected` repeats C-206's conflation

## Goal

Make the host's per-connector view answer *"is there anything for me to do here?"* — a question a
single boolean cannot answer.

## What was measured

`crates/connectors-api/src/api.rs:168`:

```rust
connected: all_stored && !provider.auth.is_empty(),
```

A connector whose vendor requires no credential has `provider.auth` empty, so `connected` is
**false** — the same value a connector shows when the operator has supplied nothing and must supply
something. Two opposite situations, one boolean:

| situation | what the operator must do | `connected` today |
|---|---|---|
| credential declared, not stored | supply it | `false` |
| credential declared, stored | nothing | `true` |
| vendor needs no credential | nothing | **`false`** |

This is [C-206](C-206-no-credential-conflates-withheld-with-absent.md) exactly, one layer up.
C-206 fixed it in the published catalogue, where the audience is a machine. Here the audience is a
person looking at a list of 45 connectors deciding which ones they can use — so the cost is higher,
not lower: a connector that is ready to call reads as one that is not, and the operator goes looking
for a token that does not exist.

## Why it is cheap

The view already carries the fields needed to express the third state. Each entry in `credentials`
has `stored` and `address`, and `provider.auth` being empty is the positive signal C-206 taught the
catalogue to read — via `Operation::auth` distinguishing `None` (inherits) from `Some([])`
(explicitly none). Nothing new has to be declared; the host has to stop collapsing what it already
knows.

## Acceptance

- [ ] **Failing-first test:** over the host's HTTP surface, a connector whose vendor requires no
      credential and a connector whose credential is simply unset are served **differently**. The
      test must fail before the change with both rendering identically. Name it.
- [ ] The distinction is one the UI can render as three states, not two. Whether that is an enum
      replacing `connected`, or `connected` kept for compatibility beside a new field, is the
      implementor's call — but a consumer must not have to infer the third state by correlating
      `connected` with the length of `credentials`.
- [ ] Read C-206's shape first and follow it rather than inventing a second vocabulary for the same
      distinction. The published catalogue now carries a `no-credential-required` note; the host
      restating that with different words is how two surfaces drift.
- [ ] `crates/connectors-api/src/index.html` renders the third state — the operator sees "nothing to
      supply", not a connector that looks unconfigured forever.
- [ ] The existing guarantee in `crates/connectors-api/tests/host.rs` is unchanged and re-proved: no
      credential value reaches any served surface, including on error.
- [ ] The gate is green.

## Notes

- Sequencing: this lands in `crates/connectors-api`, which also hosts
  [C-204](C-204-google-signin-accounts.md) and [C-207](C-207-the-host-forgets-every-credential.md).
  Those two are the goal's critical path; this one is small and can follow either.
- Nothing in the shipped catalogue declares `auth = []` yet, so like C-206 this is **latent** until
  the first genuinely-public connector lands ([C-133](C-133-provider-brave-talk-tokens.md) and
  C-157 are the candidates). Fixing it before that arrives is the cheap ordering — the alternative is
  discovering it through a connector that looks broken.
- The related gap the C-206 implementor also recorded:
  `crates/connector-cli/src/catalog.rs::credential_mechanisms` returns `[]` for both a withheld and a
  public operation, so the published `operation.credentials` field still conflates them even though
  `status` no longer does. That is a catalogue-side fix, not a host-side one, and is not this story.
