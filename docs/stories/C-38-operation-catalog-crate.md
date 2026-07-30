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
- [x] **The final build artifact stays one `.flux` per provider** — `connectors/<name>.flux`,
      unchanged in role. Per-operation renderings are additional, not a substitution.
      *Deleting all six and rebuilding reproduces them byte for byte;
      `catalog_artifacts.rs::every_rendering_is_the_text_the_shipped_module_carries` pins each
      rendering as a substring of the module that ships.*
- [x] `flux-connectors build` also writes one `.flux` rendering per operation, keyed by its address,
      as the catalog's source.
      *Written to `crates/catalog/ops/<provider>/<id>.flux` and keyed on `Operation::id`, not on an
      address — the `oip` is C-37's and does not exist yet. The key is a type
      (`catalog::OperationKey`), not a bare `&str`, so the address lands as a constructor.*
- [x] `crates/catalog` embeds every rendered operation at compile time (`include_str!` or equivalent
      generated module) — no filesystem lookup at runtime, so a consumer gets the catalog by adding
      the crate.
      *`crates/catalog/src/generated/<provider>.rs`, one `include_str!` per operation. The crate has
      no dependencies at all.*
- [ ] The catalog is queryable **by `oip`** (C-37), returning the Flux source plus the metadata a
      caller needs to decide whether to use it: risk, idempotency, the credentials required, and the
      hosts it reaches.
      *The metadata half is done — `catalog::Operation` carries `flux`, `risk`, `idempotency`,
      `credentials` (alternatives of AND-mechanisms) and `hosts`. Querying by `oip` waits on C-37;
      today the key is `OperationKey::id`.*
- [ ] Listing by `pid` and by `gid` works, so "every operation in this provider" and "every operation
      in this group" are one call each — that is what makes the middle addressing level earn its
      keep.
      *Listing by provider works — `catalog::operations_of(ProviderKey::id("zendesk"))`. Neither
      `pid` nor `gid` exists until C-37, so neither can be keyed on yet.*
- [ ] The generated catalog module is a **checked artifact**: `flux-connectors check` fails when it
      is stale, and a rebuild from unchanged inputs is byte-identical.
      *Byte-identity and staleness are asserted by
      `crates/connector-cli/tests/catalog_artifacts.rs` (four tests, including
      `the_committed_tree_is_a_fixed_point_of_a_build`), and `flux-connectors diff` reports a stale
      catalog exactly as it reports a stale module. `flux-connectors check` itself is C-14's and
      still bails as unimplemented, so this stays unticked.*
- [x] Every embedded operation parses and analyzes (the C-11 gate applies to each one individually,
      not only to the assembled provider module).
      *`crates/catalog/tests/embedded_operations.rs` parses, formatter-checks and loads each of the
      25 embedded renderings on its own.*

## Progress
- **C-38 implemented, less the parts that depend on C-37.** `flux-connectors build` now plans 34
  artifacts instead of 6: the six that ship, plus one `.flux` rendering per operation under
  `crates/catalog/ops/<provider>/` and one generated table per provider under
  `crates/catalog/src/generated/`. All of it travels through `pipeline::plan`, so the existing
  properties — nothing written until everything compiles, unchanged files untouched, `diff` reports
  drift — cover the catalog for free.
- **`crates/catalog` (package `connector-catalog`, lib `catalog`) has no dependencies.** The
  metadata is a generated `static` table rather than data parsed at runtime, so adding the crate
  costs a consumer one crate and no initialization.
- **Where C-37 plugs in.** `OperationKey` and `ProviderKey` are opaque, constructed only through
  named constructors and with no `From<&str>`; C-37 turns each private field into an enum and adds
  `OperationKey::oip` / `ProviderKey::pid` plus a sibling `GroupKey`, without moving a signature.
  The `gid` is also the natural second directory level under `ops/<provider>/`, which is what keeps
  a 163-operation babelforce navigable.
- **`src/generated.rs` is hand-written** — one `mod` line and one `PROVIDERS` entry per provider —
  because `build --provider zendesk` must not have to drop the other two from a generated index.
  `embedded_operations.rs::the_provider_list_matches_the_repository` fails when the list and
  `providers/` disagree.

## Notes
- **"Expanded" means post-overlay.** Today the hand-authored provider TOML *is* the expanded form, so
  this works without spec ingest. Once C-4/C-6 land, the same emitter runs over spec-derived
  operations with no change here.
- **Settled: the final build artifact is one `.flux` per provider.** Per-operation renderings are
  the **catalog's** unit and an intermediate of the build — they are not the deliverable. The
  per-provider module is what ships and what installs into `~/.flux/flows`, which is what
  `connectors/<name>.flux` already is today (C-27). Do not replace it, and do not make the
  per-operation files the thing a user installs.
- **Why a crate rather than files:** it makes flux-connectors consumable with `cargo add` instead of
  by copying artifacts into `~/.flux/flows`, and it stays inside the charter — a library that hands
  out text, not a runtime. Contrast with the [connectors proxy](../designs/connectors-proxy.md),
  which is gated on a charter decision precisely because it *is* a runtime.
- **Watch the file count.** 25 operations today across three providers; a spec-ingested babelforce
  alone offers 163. Per-operation files scale linearly with what we select, which is another reason
  selection stays opt-in (C-6).
- Adding `crates/catalog` to the workspace is a root `Cargo.toml` edit, so this story cannot share a
  wave with anything else that touches a manifest.
