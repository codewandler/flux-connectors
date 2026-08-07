---
id: C-521
title: "Move the flux engine line to 0.58"
pillar: "Core"
status: backlog
areas: [build, release]
note: "post-Milestone-1: registry preflight on all seven codewandler-flux-* crates, then all six engine pins move together and the four-crate closure releases as 0.21.0; flux-spec 1.x is checked separately"
---

# Move the flux engine line to 0.58

## Goal

Move the six authored `codewandler-flux-*` engine pins from 0.54 to 0.58 as one line and release
the four-crate publish closure as 0.21.0, so flux-exchange can adopt one runtime line without a
second engine trait graph. This is scheduled after the active Milestone 1 release train: nothing on
the X-134/X-126 critical path consumes 0.55–0.58, and the flux 0.55–0.58 library deltas verified
against this repository's consumed surface show no `flux_runtime::Tool`, `ToolContext`,
`flux_core::Error` or flux-lang parse/format/AST break in range.

## Acceptance

- [ ] All seven `codewandler-flux-*` engine crates this workspace requires resolve at 0.58.0 on
      crates.io before the first manifest edit; the registry preflight is recorded in Progress with
      the publication workflow evidence. (At filing time flux-web, flux-plugin, flux-tools,
      flux-app, flux-channels and flux-flow sat at 0.57.0 from a failed idempotent publish run.)
- [ ] Failing first: moving only one authored engine pin makes
      `every_flux_requirement_states_the_recorded_line` fail naming the exact 0.54/0.58 mismatch.
- [ ] All six authored engine requirements and `ENGINE_LINE` move together;
      `the_lock_carries_one_engine_line` proves `Cargo.lock` resolves exactly one 0.58 engine line
      with no transitive second line via flux-plugin or flux-credentials. The independent
      `flux-spec` line is checked separately against the published 1.4 line and `SPEC_LINE` moves
      only if the resolved graph requires it.
- [ ] Generated artifacts are rebuilt and either byte-identical or the formatter drift is reviewed:
      any change to `connectors/*.flux`, `crates/catalog/ops/**/*.flux` or `connectors.lock` hashes
      is explained by a named flux-lang change (the only formatter change in range is the additive
      JSON-quoted field-path syntax), and `catalog_artifacts.rs` plus the provider round-trip
      tests are green.
- [ ] The `WebOptions::default()` private-network posture and the `http.request` result shape are
      re-verified against 0.58 (`live_egress.rs`), since both can move without a compile error.
- [ ] The complete four-crate publish closure packages with `cargo publish --dry-run` only; full
      workspace, generated-artifact, site and host-page gates are green before the 0.21.0 minor is
      tagged and published through CI.
