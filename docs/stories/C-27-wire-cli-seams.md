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
- [x] `connector_cli::seam::load` (`crates/connector-cli/src/seam.rs:111`, marker at `:93`) calls
      C-3's provider-TOML loader. Its signature is already bytes-in / IR-out; the return type
      `seam::Connector` becomes `connector_spec::Connector`.
- [x] `connector_cli::seam::emit` (`crates/connector-cli/src/seam.rs:161`, marker at `:136`) calls
      C-8's Flux emitter.
- [x] `emit` stays **deterministic**, **total** (both artifacts or neither) and **returns text
      rather than writing** — C-13's byte-identical-no-op and atomic-write guarantees all rest on
      those three properties.
- [x] The private FNV-1a `Digest` in `seam.rs` is removed; it exists only so placeholder artifacts
      vary with their inputs. The real hash is C-7's sha256.
- [x] `flux-connectors build` at the repo root produces artifacts that pass C-11's
      parse-and-analyze gate. *(Asserted by shape from this crate — see Progress.)*

## Progress
- **Wired (C-27).** `load` is `connector_spec::provider::load(providers/<name>.toml, bytes).connector`;
  `emit` builds the module from `connector_flux::emit_operation` per operation under a `#`-comment
  header, and derives the manifest from the IR. The FNV-1a `Digest` is gone.
- **`load` refuses a `[spec]`-backed provider for now.** Spec ingest is C-4 and is not wired, so such
  a file loads as a skeleton with no operations and would emit a module that *parses cleanly and
  declares nothing* — verified out of band: a header-only module parses as a program with 0 ops.
  Failing loudly is the lesser evil; C-4 removes the refusal.
- **Parse-and-analyze cannot be asserted from `connector-cli`.** The crate does not depend on
  `flux-lang` and `connector-flux` re-exports none of it, so `flux_lang::program::Module::parse_str`
  is unreachable without a manifest edit. `tests/wiring.rs` pins the module envelope by shape
  (`#` comments, never `//`); the real gate is C-11's, which should live where flux-lang is already
  a dependency.
- **The `.connector.toml` manifest is still this crate's, not `connector-flux`'s.** C-8 emits Flux
  only. What `seam::manifest` writes today is what the IR knows — connector, vendor, base URL,
  module file, operation ids. `http_hosts`, the endpoint env spec and the credential declarations
  are C-10.
- Nothing outside `seam.rs` changed: no caller inspects a `Connector` or knows how artifact text is
  produced, exactly as C-13 predicted. The integration *fixtures* had to become real connector
  definitions, because the placeholder loader accepted `id = "acme"` and the real one validates.

## Notes
- **Sequencing: C-8 must land before C-17.** C-13's placeholder `.flux` output is not valid
  op-bearing Flux. Harmless while no real providers exist, but if C-17 authors real provider TOMLs
  before the emitter is wired, `build` will emit modules that C-11's gate must reject.
- Artifact output directory is `connectors/<name>.flux` and `connectors/<name>.connector.toml` —
  chosen by C-13 because nothing in the design specified where *committed* artifacts live (the design
  names only the install destinations). It is confined to `workspace.rs` if it should change.
