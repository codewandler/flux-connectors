---
id: C-540
title: "Retire connector-flux and the compiled Flux artifacts"
pillar: Build
status: backlog
priority: 2
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-flux, catalog, artifacts, docs]
note: "Delete the emitter, connectors/*.flux, catalog/ops/**, and the parse-back halves of connector-pack — in the same release train as proven adoption, per the Decision 0022 migration rule"
---

# Retire connector-flux and the compiled Flux artifacts

## Goal

Delete the Flux emission path once — and only once — the differential gate holds and Exchange
consumes the artifact, completing Decision 0022 in the same release train as the proven
replacement.

## Acceptance

- [ ] Preconditions verified in-session, commands quoted in this story's Progress: C-538's
      differential gate green across the whole catalogue, and C-539's adoption evidence recorded.
- [ ] Removed from the workspace: `crates/connector-flux`, `connectors/*.flux`,
      `crates/catalog/ops/**`, the generated Rust storage in `crates/catalog/src/generated/`
      (per C-537's disposition), and the parse-back halves of `connector-pack` (`spec.rs`, the
      AST walk in `request.rs`) together with the differential gate itself.
- [ ] Every test that read the deleted artifacts from disk (`connector-pack/tests/differential.rs`,
      `request.rs`'s manifest enumeration, the rehearsal tests) is re-pointed at the canonical
      documents or deleted with its assertion moved, never silently dropped.
- [ ] `connector-cli diff` reports a consistent artifact count; the README/AGENTS declared counts
      are regenerated and `readme_snippet.rs` passes.
- [ ] The `.connector.toml` projection and `web/public/catalog.json` continue to be emitted from
      the canonical documents; the explorer renders the request template where it rendered Flux.
- [ ] `CHANGELOG.md` and `WHATS-NEW.md` state the removal and the exact consumer action (none, if
      the shims held their API line).

## Progress

- (not started)

## Notes

- Blocked on C-538 and C-539. Nothing in this story may land before both; a partial deletion that
  leaves the catalogue claiming a `.flux` it no longer ships is the failure mode to refuse.
- Deliberately **not** gated on X-151: the Tool-returning wrapper and the engine-line machinery
  retire in C-541, whose gate is Exchange's plan-API adoption. Two deletions, two adoption proofs,
  two release trains — this story must not stall behind a sibling repo's merge queue.
- `flux_lang` may remain a dev-dependency wherever a test still asserts about historical
  renderings; the production dependency edge from the resolve path must be gone.
