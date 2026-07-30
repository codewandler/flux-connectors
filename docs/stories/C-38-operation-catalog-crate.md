---
id: C-38
title: Render Flux per operation and embed it in a catalog crate
pillar: Build
status: ready
priority: 6
design: docs/designs/global-addressing.md
epic: connectors-v1
areas: [connector-flux, connector-cli, catalog]
note: **the goal** · one .flux per expanded op, consumable as a Rust crate
---

# Render Flux per operation and embed it in a catalog crate

## Goal
Emit one `.flux` rendering per **expanded** operation rather than only a per-provider module, and
embed those renderings in a `crates/catalog` crate — so connectors become consumable as a Rust
dependency, addressable per operation, instead of as loose files a user must install.

## Acceptance
- [ ] `flux-connectors build` writes one `.flux` per operation, keyed by its address, in addition to
      the per-provider module.
- [ ] `crates/catalog` embeds every rendered operation at compile time (`include_str!` or equivalent
      generated module) — no filesystem lookup at runtime, so a consumer gets the catalog by adding
      the crate.
- [ ] The catalog is queryable **by `oip`** (C-37), returning the Flux source plus the metadata a
      caller needs to decide whether to use it: risk, idempotency, the credentials required, and the
      hosts it reaches.
- [ ] Listing by `pid` and by `gid` works, so "every operation in this provider" and "every operation
      in this group" are one call each — that is what makes the middle addressing level earn its
      keep.
- [ ] The generated catalog module is a **checked artifact**: `flux-connectors check` fails when it
      is stale, and a rebuild from unchanged inputs is byte-identical.
- [ ] Every embedded operation parses and analyzes (the C-11 gate applies to each one individually,
      not only to the assembled provider module).

## Progress
- (not started)

## Notes
- **"Expanded" means post-overlay.** Today the hand-authored provider TOML *is* the expanded form, so
  this works without spec ingest. Once C-4/C-6 land, the same emitter runs over spec-derived
  operations with no change here.
- **Per-operation files complement the per-provider module; they do not replace it.** flux's
  `DynamicComposites::load` lifts every `op` declaration out of every `.flux` file in
  `~/.flux/flows`, so either shape loads. The per-provider module stays the **installable** unit; the
  per-operation renderings are the **catalog's** unit and give per-op diffs and selective use.
  Confirm this reading before building — collapsing to one shape is cheaper if the module is not
  wanted.
- **Why a crate rather than files:** it makes flux-connectors consumable with `cargo add` instead of
  by copying artifacts into `~/.flux/flows`, and it stays inside the charter — a library that hands
  out text, not a runtime. Contrast with the [connectors proxy](../designs/connectors-proxy.md),
  which is gated on a charter decision precisely because it *is* a runtime.
- **Watch the file count.** 25 operations today across three providers; a spec-ingested babelforce
  alone offers 163. Per-operation files scale linearly with what we select, which is another reason
  selection stays opt-in (C-6).
- Adding `crates/catalog` to the workspace is a root `Cargo.toml` edit, so this story cannot share a
  wave with anything else that touches a manifest.
