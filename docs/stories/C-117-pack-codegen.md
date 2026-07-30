---
id: C-117
title: "Generate the pack from the IR and hold it to the drift gate"
pillar: Codegen
status: ready
priority: 4
design: docs/designs/connector-tool-pack.md
epic: tool-pack
areas: [codegen, bridge]
note: "two surfaces from one IR can disagree about the same operation — the differential test is the honest guard, and it belongs here rather than in a later postmortem"
---

# Generate the pack from the IR and hold it to the drift gate

## Goal

Emit the Tool pack from the same IR and in the same build as `connectors/*.flux`, and prove the two
surfaces agree about every operation.

## Acceptance

- [ ] `cargo run -p connector-cli -- build` emits the pack's generated source alongside the existing
      artifacts, and the committed tree stays a **fixed point** — a second build writes nothing.
- [ ] `the_committed_tree_is_a_fixed_point_of_a_build` covers the new artifacts. This is the guard
      that already caught an unregenerated tree at `1bea397`, so it must see the pack too.
- [ ] **Failing-first test:** `the_pack_and_the_module_agree_about_every_operation` — for every
      shipped operation, construct the pack's request and compare method, URL and body shape against
      the emitted `.flux` module's. A divergence fails, naming the operation.
- [ ] The generated pack respects the `catalog.json` full-build-only rule: a provider-scoped build
      must not rewrite a global index from partial data. Follow whatever
      [C-104](C-104-catalog-fanout-enabler.md) settled for `crates/catalog/src/generated.rs`.
- [ ] No credential value, and no hand-maintained provider list, enters the generated source.
- [ ] The gate is green, and `cargo run -p connector-cli -- diff` reports no drift.

## Notes

- **The risk this story exists to contain:** two surfaces generated from one IR can drift into
  disagreeing about the same operation. `AGENTS.md` already warns about exactly this for the C-12 /
  C-95 shared lowering. The differential test is not optional polish — without it, the pack and the
  module can send different requests for one connector and both look correct in isolation.
- Prefer sharing the request-construction code between the pack and the emitter over duplicating it.
  Two code paths that must agree are strictly worse than one path with two renderings.
- If the generated pack turns out to be large, check what it does to build times before assuming a
  per-operation type is the right shape — a data-driven Tool parameterised by a catalogue entry may
  be better than 236 generated structs. Measure rather than guess, and record the finding.
