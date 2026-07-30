---
id: C-130
title: "The ivr service and its atomic operation inventory"
pillar: Spec
status: ready
priority: 3
design: docs/designs/babelforce-ivr-atomics.md
epic: babelforce-ivr
areas: [providers, connector-spec]
note: "six composable parts beat seventeen frozen combinations — audioplayer, read, switchnode, dial, recording, acd. Scope agentic and realtime out until the plain six land"
---

# The ivr service and its atomic operation inventory

## Goal

Add `service = "ivr"` with `api_version = "2"` to `providers/babelforce.toml` and publish the atomic
call primitives as operations.

## Acceptance

- [ ] `[[services]]` gains `ivr` at `api_version = "2"`, alongside the existing surfaces. babelforce
      currently declares no services, so this story also places the existing `agent` and `call`
      operations into their own service — the loader refuses a file that declares any service while
      leaving an operation unassigned ([C-49](C-49-provider-services.md)).
- [ ] The **inventory is written down before any TOML** — a table mapping each `internal/modules/*`
      primitive to the operation(s) it becomes, with the vendor endpoint each one calls. Record it in
      the design doc. An inventory derived while editing TOML is an inventory nobody can review.
- [ ] Operations for the plain atomics: `audioplayer`, `read`, `switchnode`, `dial`, `recording`,
      `acd`. Each carries declared parameters, `risk` and `idempotency` chosen deliberately — `dial`
      places a real call and costs real money, and its risk must say so.
- [ ] **No call module is published.** `simpleMenu`, `promptPlayer` and friends are compositions; a
      test or an explicit note records that they were considered and excluded, so a later contributor
      does not "helpfully" add them.
- [ ] `agentic` and `realtime` are **out of scope** — both appear to carry streaming or model
      semantics a request/response operation cannot express. Say so in Progress rather than guessing
      at them.
- [ ] Generated Flux parses, analyzes and is a fixed point of flux's own formatter — the standing
      per-provider gate.
- [ ] No credential value anywhere, and no `example` on a `secret` field that looks like a real token.
- [ ] The build stays a fixed point and the full gate is green.

## Notes

- **Read the source, not the API docs alone**: `~/babelforce/projects/ivr/ivr/internal/modules/`.
  `flows/*.yaml` shows how each module is actually configured in practice, which is better evidence of
  the real parameter set than any document.
- A connector **selects** the operations worth exposing (`vision.md`). If a primitive has twelve
  settings and two matter, expose two and say why.
- This story is deliberately operations-only. Events are [C-131](C-131-ivr-events.md); do not fold
  them in, because the event set has a naming problem that deserves its own review.
- Whole-catalogue artifacts are coordinator-owned as of C-104 — use a provider-scoped build as your
  gate and do not hand-edit a global index.
