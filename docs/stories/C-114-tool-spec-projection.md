---
id: C-114
title: "The connector-pack crate and the ToolSpec projection"
pillar: Bridge
status: ready
priority: 1
design: docs/designs/connector-tool-pack.md
epic: tool-pack
areas: [bridge]
note: "the foundation the rest of the epic builds on — a catalogue entry becomes a flux ToolSpec, dotted name and all"
---

# The connector-pack crate and the ToolSpec projection

## Goal

Create `crates/connector-pack` and project a catalogue operation onto a flux `ToolSpec`, so that a
host can register a provider's operations into a `ToolRegistry` and see them resolve by their dotted
names.

This story delivers the **declaration** half only. Executing a request is C-115; this one must
compile, register, and pass a spec-shape test without ever issuing a call.

## Acceptance

- [ ] `crates/connector-pack` exists, depends on `catalog` and on flux's `flux-runtime` / `flux-spec`
      at the version the workspace already pins, and is added to the workspace members.
- [ ] `connector_pack::pack(&["zendesk"])` returns a value usable as
      `ClientBuilder::try_register_pack`'s argument — i.e. `FnOnce(&mut ToolRegistry)`.
- [ ] **The dotted name is derived, not hand-written.** `zendesk-ticket-comment-add` projects to
      `zendesk.ticket.comment.add`. The mapping is one function with its own unit tests, because it
      is the seam flux's reference flow depends on.
- [ ] The projection carries `risk`, `idempotency`, `effects`, `description` and `input_schema` from
      the catalogue entry onto `ToolSpec`. A field the IR does not have is `None`/empty rather than
      invented.
- [ ] Registering two providers whose operations collide surfaces flux's own duplicate diagnostic
      rather than panicking — use `try_register_all_from` with a per-provider source label.
- [ ] **Failing-first test:** `every_shipped_operation_projects_to_a_registrable_spec` — iterate the
      real catalogue, project each operation, register the lot, and assert the registry resolves each
      dotted name. It must fail before the projection exists.
- [ ] The gate is green: `cargo fmt --all && cargo build --workspace && cargo test --workspace &&
      cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`.

## Notes

- Read `crates/flux-spec/src/lib.rs:289` for `ToolSpec`'s exact fields, and
  `crates/flux-runtime/src/lib.rs` for `ToolRegistry::try_register_all_from`, which installs a pack
  atomically under one auditable source label.
- The catalogue is the input, not `providers/*.toml`: `catalog::operations_of`, `catalog::providers`
  (`crates/catalog/src/lib.rs`). That keeps this crate free of the loader.
- `execute` may be `unimplemented!()` in this story **only if** no test can reach it. If that is
  awkward, return a clear "not yet wired" error instead — a panic reachable from a host is worse than
  an error.
- Do **not** add a `flux-sdk` dependency here. The pack must be usable by a bare `ToolRegistry`;
  the SDK is the host's choice, not ours.
