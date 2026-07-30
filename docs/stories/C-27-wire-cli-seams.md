---
id: C-27
title: Wire the CLI seams to the loader and the emitter
pillar: Build
status: blocked
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-cli]
note: two functions · mechanical once C-3 and C-8 land
---

# Wire the CLI seams to the loader and the emitter

## Goal
Replace C-13's two placeholder seam functions with the real provider-TOML loader and the real Flux
emitter, so `flux-connectors build` produces genuine artifacts instead of deterministic placeholders.

## Acceptance
- [ ] `connector_cli::seam::load` (`crates/connector-cli/src/seam.rs:111`, marker at `:93`) calls
      C-3's provider-TOML loader. Its signature is already bytes-in / IR-out; the return type
      `seam::Connector` becomes `connector_spec::Connector`.
- [ ] `connector_cli::seam::emit` (`crates/connector-cli/src/seam.rs:161`, marker at `:136`) calls
      C-8's Flux emitter.
- [ ] `emit` stays **deterministic**, **total** (both artifacts or neither) and **returns text
      rather than writing** — C-13's byte-identical-no-op and atomic-write guarantees all rest on
      those three properties.
- [ ] The private FNV-1a `Digest` in `seam.rs` is removed; it exists only so placeholder artifacts
      vary with their inputs. The real hash is C-7's sha256.
- [ ] `flux-connectors build` at the repo root produces artifacts that pass C-11's
      parse-and-analyze gate.

## Progress
- **Blocked on C-3 and C-8.** C-13 landed the whole orchestration around these two functions:
  discovery, planning, atomic artifact writing, the byte-identical no-op, `--provider` filtering,
  `diff`, and the offline guarantee.
- Nothing outside `seam.rs` inspects a `Connector` or knows how artifact text is produced, so no
  caller changes are expected.

## Notes
- **Sequencing: C-8 must land before C-17.** C-13's placeholder `.flux` output is not valid
  op-bearing Flux. Harmless while no real providers exist, but if C-17 authors real provider TOMLs
  before the emitter is wired, `build` will emit modules that C-11's gate must reject.
- Artifact output directory is `connectors/<name>.flux` and `connectors/<name>.connector.toml` —
  chosen by C-13 because nothing in the design specified where *committed* artifacts live (the design
  names only the install destinations). It is confined to `workspace.rs` if it should change.
