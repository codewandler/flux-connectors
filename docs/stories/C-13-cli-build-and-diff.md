---
id: C-13
title: Build and diff from the vendored spec cache
pillar: Build
status: ready
priority: 9
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-cli]
---

# Build and diff from the vendored spec cache

## Goal
Give the repo its build command: compile every provider from committed inputs into committed
artifacts, hermetically and offline, with a `diff` that previews what would change.

## Acceptance
- [ ] `flux-connectors build` reads `providers/*.toml` and `specs/<provider>/<version>.json` and
      writes `<provider>.flux`, `<provider>.connector.toml`, and `connectors.lock`.
- [ ] The build performs **no network IO** — proven by a test that runs it with networking
      unavailable.
- [ ] `flux-connectors diff` shows what a rebuild would change without writing anything.
- [ ] Building twice from unchanged inputs is a no-op producing byte-identical artifacts.
- [ ] `--provider <name>` restricts the build to one connector.

## Progress
- (not started)

## Notes
- Hermetic-and-offline is the point: the vendored spec cache is committed so a build is reproducible
  and reviewable years later.
- Deliberately **not** a `build.rs`. Generation is an explicit step whose output a human reads as a
  diff in a PR.
