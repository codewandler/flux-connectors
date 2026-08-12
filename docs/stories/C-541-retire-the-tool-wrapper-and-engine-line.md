---
id: C-541
title: "Retire the Tool wrapper and the engine-line machinery"
pillar: Build
status: backlog
priority: 2
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-pack, connector-cli, release]
note: "Gated on Exchange's plan-API adoption (X-151): delete connector-pack's Tool-returning wrapper and its codewandler-flux-* dependencies in the same change as flux_engine_line.rs — a pin constant must not outlive its constraint"
---

# Retire the Tool wrapper and the engine-line machinery

## Goal

Complete the engine-line half of Decision 0022: once Exchange consumes the plan API, delete
`connector-pack`'s Tool-returning wrapper and its `codewandler-flux-*` dependencies together with
the machinery that exists only to manage that coupling — in one change, so no test keeps enforcing
a constraint nobody is bound by.

## Acceptance

- [ ] Precondition verified in-session, command quoted in Progress: X-151 landed on Exchange's
      main and its release consumes the plan API (no `connector_pack::resolve`-returning-`Tool`
      call site remains there).
- [ ] `connector-pack`'s Tool-returning wrapper (`resolve`/`project`/`pack` returning
      `flux_runtime` types) is deleted; the engine-free plan core is the crate's whole public
      surface, and no `codewandler-flux-*` dependency remains in the publish closure
      (`scripts/publish-crates-io.sh --print-order` + `cargo tree` evidence quoted).
- [ ] `crates/connector-cli/tests/flux_engine_line.rs` (`ENGINE_LINE`/`SPEC_LINE`; 242 lines,
      3 `#[test]` fns — `wc -l` / `grep -c '#[test]'`, measured 2026-08-12) is retired, or
      re-scoped to whatever still links the engine, **in the same change** — with a note in the
      file or its replacement naming this story so the constraint's end is discoverable.
- [ ] The workspace pin comments in `Cargo.toml` describing the moved-as-one-line rule are
      updated or removed to match reality.
- [ ] `CHANGELOG.md` and `WHATS-NEW.md` state the consumer action for anyone still on the wrapper
      (there should be none — name the check that proved it).

## Progress

- (not started)

## Notes

- Split out of C-540 on 2026-08-12: the `.flux`-emission deletion is gated on C-538+C-539 (artifact
  adoption), this deletion on X-151 (plan-API adoption). The chain
  C-538 → X-151 → C-541 resolves — C-538 waits on nothing in Exchange — but it means this story
  can never share a wave or a release train with C-538, by design, per the Decision 0022 migration
  rule: each deletion lands with its own replacement's proven adoption.
- The Exchange-side mirror (its `ENGINE_LINE` constant and engine-line tests) is X-151's to retire;
  the two copies of the one constraint must each die in their own repo's adoption change, and
  neither side may outlive the other silently — that is folklore with a passing test behind it.
