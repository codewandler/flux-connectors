# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Repository scaffolding: the track backlog framework (vision, roadmap, stories board, design
  records) and the initial `connectors-v1` epic.
- **C-1** — the three-crate Cargo workspace (`connector-spec`, `connector-flux`, `connector-cli`
  producing the `flux-connectors` binary), dual MIT/Apache-2.0 licences, `.gitignore`, a README, and
  a CI workflow running build, test, `clippy -D warnings` and `fmt --check`.
- **C-1** — a flux-lang smoke test (`crates/connector-flux/tests/flux_lang_smoke.rs`) parsing a
  trivial `.flux` source through `flux_lang::program::Module::parse_str`, proving the dependency
  resolves and its API is usable from a consumer crate.

- **C-2** — the Connector IR in `connector-spec`: `Connector`, `Operation`, `ParamSet`, `Param`,
  `Quirks`, `Provenance`, plus the auth model (`AuthMethod`, `AuthRequirement`, `AuthScheme`,
  `OAuth2Spec`). Parameters and responses carry their JSON Schema; `risk` and `idempotency` are
  mandatory, so neither can be decided by silence.
- **C-2** — multi-credential auth: an operation references requirement *sets* — all credentials in a
  set (AND), any one set among alternatives (OR) — with unset and explicitly-empty distinguishable
  **on the wire**, so an unauthenticated operation cannot silently inherit credentials.
- **C-2** — deterministic IR serialization, proven from four angles including a tripwire that fails
  if anything in the workspace ever enables `serde_json/preserve_order`.
- **C-18** — `docs/designs/provider-operation-inventory.md`: curated operation sets and auth models
  for zendesk (7 of 7), freshdesk (9 of 16) and babelforce (9 of 163), every claim carrying a
  `path:line` citation.

- **C-16** — the `$auth` seam design, verified line-by-line against flux `v0.38.0`, plus eleven
  paste-ready story drafts for flux's board (`docs/designs/auth-seam-flux-stories.md`).
- **unified-auth epic** — credentials modelled on three orthogonal axes (source × acquisition ×
  placement) so a new provider archetype costs one value on one axis, not a new variant crossing all
  of them. Stories C-19 … C-22.

### Changed
- **C-1** — flux-lang is depended on from **crates.io** (`codewandler-flux-lang = "0.37"`) rather
  than as a git or path dependency. The flux git remote uses a developer-only SSH host alias that
  cannot resolve in CI, and a `../flux` path dependency is absent from a fresh clone.
