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
      *Discovery, reading and writing are done — both artifacts land in `connectors/`. Not ticked
      for two reasons: `connectors.lock` belongs to `C-7`, and the artifact **contents** are still
      placeholders until `C-3` and `C-8` fill the two seam functions.*
- [x] The build performs **no network IO** — proven by a test that runs it with networking
      unavailable.
- [x] `flux-connectors diff` shows what a rebuild would change without writing anything.
- [x] Building twice from unchanged inputs is a no-op producing byte-identical artifacts.
- [x] `--provider <name>` restricts the build to one connector.

## Progress
- **Orchestration landed; the two compiler stages are stubbed and clearly marked.** `build`, `diff`,
  `check`, `fetch` and `install` all exist as commands; `build` and `diff` are real, and the other
  three fail loudly naming the story that lands them rather than exiting zero.
- **The two wiring points** are `connector_cli::seam::load` (C-3's provider-TOML loader) and
  `connector_cli::seam::emit` (C-8's Flux emitter), both in `crates/connector-cli/src/seam.rs`.
  Nothing outside that module inspects a `Connector` or knows how artifact text is produced, so
  connecting the real crates is a change to those two bodies and the placeholder types beside them.
- **Layout chosen:** inputs at `providers/<name>.toml` and `specs/<name>/<version>.*`, artifacts at
  `connectors/<name>.flux` and `connectors/<name>.connector.toml`. The repo has no `providers/`
  directory yet (`C-17` authors those), so every test builds a fixture tree under a temp root and
  passes it with `--root`.
- **How "no network" is proven** (`crates/connector-cli/tests/no_network.rs`), three ways: the build
  runs with `net::deny()` armed and never reaches the seam; a source audit asserts no network
  primitive exists outside `src/net.rs`, so the counter cannot be bypassed; and the shipped binary
  builds successfully inside an empty network namespace (`unshare --user --map-root-user --net`),
  which skips loudly where unprivileged netns are unavailable. The audit was verified to have teeth
  by temporarily adding a `TcpStream::connect` and watching it fail.
- **Two atomicity levels**, since a half-written artifact is a diff a human might approve:
  per-file via write-to-temp-then-rename, and per-run because `pipeline::plan` compiles every
  provider before `pipeline::apply` writes any file.
- **No new dependency was added.** The argument parser is hand-rolled in `src/cli.rs` (~5 flags)
  rather than blocking on a `clap` dependency that would collide with C-3 and C-8 in flight.
- Next: fill the two seam functions, then `C-7` for `connectors.lock` and `C-14` for `check`.

## Notes
- Hermetic-and-offline is the point: the vendored spec cache is committed so a build is reproducible
  and reviewable years later.
- Deliberately **not** a `build.rs`. Generation is an explicit step whose output a human reads as a
  diff in a PR.
