---
id: C-114
title: "The connector-pack crate and the ToolSpec projection"
pillar: Bridge
status: in-progress
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

- [x] `crates/connector-pack` exists, depends on `catalog` and on flux's `flux-runtime` / `flux-spec`
      at the version the workspace already pins, and is added to the workspace members.
- [x] `connector_pack::pack(&["zendesk"])` returns a value usable as
      `ClientBuilder::try_register_pack`'s argument — i.e. `FnOnce(&mut ToolRegistry)`.
- [x] **The dotted name is derived, not hand-written.** `zendesk-ticket-comment-add` projects to
      `zendesk.ticket.comment.add`. The mapping is one function with its own unit tests, because it
      is the seam flux's reference flow depends on.
- [x] The projection carries `risk`, `idempotency`, `effects`, `description` and `input_schema` from
      the catalogue entry onto `ToolSpec`. A field the IR does not have is `None`/empty rather than
      invented.
- [x] Registering two providers whose operations collide surfaces flux's own duplicate diagnostic
      rather than panicking — use `try_register_all_from` with a per-provider source label.
- [x] **Failing-first test:** `every_shipped_operation_projects_to_a_registrable_spec` — iterate the
      real catalogue, project each operation, register the lot, and assert the registry resolves each
      dotted name. It must fail before the projection exists.
- [x] The gate is green: `cargo fmt --all && cargo build --workspace && cargo test --workspace &&
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

## Progress

Landed on `impl/C-114`. `crates/connector-pack` is a workspace member; `pack(&[…])` returns
`FnOnce(&mut ToolRegistry) -> flux_core::Result<()>`, installing each provider under its own source
label (`connector-pack:<provider>`) via `try_register_all_from`. 97 operations across 17 providers
register and resolve by their dotted names.

Three findings worth carrying into C-115/C-117:

1. **`access` is not optional.** Leaving `ToolSpec::access` empty — which is what flux's own
   `OpSpec::lower` does, and what "a field the IR lacks is empty" argued for — makes flux **refuse
   the registration**: `authority_requirements_from_declaration` rejects a declared `Effect::Network`
   with no carrying `AccessKind`. This was caught by a test, not by reading. `spec::access_for` now
   derives access from the declared effects as an exhaustive match over `flux_spec::Effect`, and
   `project` re-runs flux's own checker so a future declaration flux would refuse is named by
   *operation id* rather than surfacing at a host's startup.

2. **The projection reads the embedded Flux, not the catalogue's flat columns.** `Operation::flux` is
   the shipped `op` declaration, and `connector-flux` already documents its metadata block as "the
   `ToolSpec` surface flux exposes to a model". Parsing it back and lowering through flux's own
   `OpSpec` makes the pack's answer *the module's answer* by construction — which is the drift C-117
   exists to guard, removed for the declaration half rather than tested for.

3. **`description` differs between the two sources, and the declaration's is the right one.**
   `Operation::description` is the raw summary; the emitter extends it with the vendor's error
   envelope. The extended text is what a model gets from the module, so it is what it must get from
   the pack. A test asserts the catalogue summary is a **prefix** of every projected description, so
   the extension can only be the emitter's.

Not this story, and left for C-115: `permission_subjects` and `intents` are still the trait's empty
defaults. That is safe only because `execute` returns `not_wired_yet` and reaches no network.
`tool::tests::the_network_gate_is_unmirrored_only_because_execute_is_inert` asserts the emptiness
deliberately, so C-115 cannot land request delegation without inverting a red test.
