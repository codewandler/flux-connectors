---
id: C-233
title: "A provider implementor cannot exercise their own connector through `connector-pack`, so 'does it compose a request at all' is unanswerable until integration"
pillar: Build
status: in-progress
priority: 1
design:
epic:
areas: [build, connector-pack, catalog]
note: "found by the C-110 implementor 2026-07-31 after its connector was withdrawn. Not an oversight it could have avoided: catalog::Operation is #[non_exhaustive] so no synthetic one can be built outside the catalog crate, and the index that carries a real one is coordinator-owned and does not hold a new provider until integration"
---

# A provider implementor cannot test through `connector-pack`

## Goal

Let a provider story answer "can this connector actually compose a request?" before integration,
rather than after.

## What was measured

C-110 shipped eight Linear operations, ran the full scoped gate green, and was found in review to
have **zero callable operations** — `connector-pack` read each pinned GraphQL document's braces as
configuration placeholders, and with configuration supplied it replaced the whole selection set:

```
{"query":"query Viewer a-viewer\n}\n"}
```

The implementor's account of why it could not have caught this is structural, not an excuse:

- every `connector-pack` entry point needs a `&'static catalog::Operation` from the index;
- `catalog::Operation` is `#[non_exhaustive]`, so no synthetic one can be constructed outside the
  `catalog` crate;
- the index is a **whole-catalogue artifact**, coordinator-owned, and does not carry a new provider
  until integration.

So the one question that matters most — does this connector work — is the one question a provider
implementor structurally cannot ask. The only defence available today is a fixture test written
inside `connector-pack`'s own crate, which is where C-110 finally put its boundary tests, and which
is outside a provider story's declared `areas`.

## Why priority 1

The gap is **not specific to GraphQL.** Any provider with a genuinely novel shape has the same blind
spot, and the wave that just landed added eight connectors on the strength of a gate that cannot see
this class of failure. The cost is asymmetric: catching it costs a test; missing it ships a connector
that looks configured and callable and is neither.

## The window that closed by luck, not design

Worth recording because it is the reason this is a story and not a postmortem.
`every_shipped_configuration_variable_is_placed`, which
[C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md) added, **would** have caught C-110 at
integration: `endpoint_slots` returns `{}` for a GraphQL declaration, since a selection-set fragment
has no request position, so every variable comes back `Unplaced` and the test goes red.

But C-214 landed *after* C-110's round-1 base. That is precisely why C-110's gate was green with
eight dead operations. The safety net arrived during the same wave by coincidence, and a coincidence
is not a mechanism.

## Acceptance

- [x] **Failing-first test:** a provider implementor, working only in their own worktree with no
      whole-catalogue artifact regenerated, can execute an operation through `connector-pack` and
      observe it refuse. Name it. Demonstrate it against C-110's withdrawn Linear documents.
      → `a_connector_that_is_not_in_the_index_can_be_rehearsed_and_refuses`
      (`crates/connector-pack/tests/rehearsal.rs`), over both `linear-viewer` and `linear-issue-get`
      exactly as the emitter wrote them. At the base the same file does not compile:
      `error[E0639]: cannot create non-exhaustive struct using struct expression`, which is the
      story's structural claim executed.
- [x] The route does **not** require regenerating the index. If the answer is a builder or a
      test-only constructor for `catalog::Operation`, say what `#[non_exhaustive]` was protecting and
      why the new route does not reopen it.
      → `connector_pack::Rehearsal::of(id, provider, service, flux)` takes the operation's **emitted
      Flux** and constructs no `catalog::Operation` at all, so `#[non_exhaustive]` keeps its full
      guarantee. What it protects is stated in `crates/catalog/src/lib.rs`: C-37's global address and
      C-10's resolved endpoint spec must be able to land as new fields without breaking a consumer,
      which holds only while no consumer can write a struct literal or destructure one exhaustively.
      A named constructor taking every field would break on the next field exactly as a literal does;
      a builder would not, but it would move the burden of keeping a synthetic entry *plausible*
      (hosts, credentials, service) onto whoever writes the test, and a wrong synthetic entry is a
      test that measures a connector nobody ships. Taking the Flux means there is nothing synthetic
      to get wrong. The reasoning is recorded in `crates/connector-pack/src/rehearsal.rs`.
- [x] The check runs inside the **scoped provider gate** `AGENTS.md` documents, so an implementor
      gets the answer without being told to look.
      → `crates/connector-pack/tests/request.rs::every_declared_operation_composes_a_request_from_its_declared_configuration`
      runs under the gate's existing `cargo test --workspace --no-fail-fast`. It enumerates
      `connectors/*.connector.toml` and `crates/catalog/ops/<provider>/`, both per-provider artifacts
      `build --provider <id>` writes, so a new connector is covered with nothing extra to run and no
      test to write.
- [x] `AGENTS.md` records it beside the eight-red-tests procedure.
      → new section "The scoped gate does answer 'can my connector make a call at all' (C-233)",
      immediately after the eight-red table and before the ninth-check section, with the two refusal
      shapes an implementor will actually see and the note that this one must be **green**.
- [x] The configuration used is what the provider **declares**, including the empty case — see
      C-232, which is the same failure from the catalogue side.
      → shared with C-232: the port carries the `[[config]]` fields' `binds` targets and `example`
      values and nothing else, and a connector declaring none is run against an empty configuration.
      `an_unconfigured_rehearsal_refuses_by_name_rather_than_composing_a_placeholder` and
      `a_value_stored_under_another_service_does_not_answer` pin the two ways that can go wrong.

## Progress

**2026-07-31 — implemented on `impl/C-232`, together with C-232.**

`Rehearsal` reaches the same code path a host does — `spec::declaration_of`,
`spec::project_declaration`, `request::endpoint_variables`/`endpoint_slots`, `Configuration::snapshot`
and `request::build` — rather than a parallel one, and
`a_rehearsal_and_a_projection_agree_on_a_shipped_operation` pins that against
`Operation::build_request` for a shipped operation so the two cannot drift.
`Snapshot`'s `provider`/`service` became owned `String`s so a connector whose names are not
`'static` reads the port through exactly the same type.

What a rehearsal deliberately does **not** cover, because it needs the catalogue rather than the
module: credential placement (the connector's `[[auth]]` table), the network gate's declared hosts,
and dotted-tool-name collisions across a whole pack. Those still arrive at integration.

**2026-07-31, round 1 rework** — the `#[non_exhaustive]` argument was judged correct and is kept,
with one overclaim corrected. "No synthetic `catalog::Operation` can be built outside the `catalog`
crate" is true of *construction* and not of *copying*: the fields are `pub`, so a shipped entry can
be cloned and overwritten, which `tests/differential.rs` does and which
`a_document_literal_is_refused_at_projection_and_not_only_at_build` now does too. It is still not a
route a provider implementor has, and `crates/connector-pack/tests/request.rs` says why where the
technique is used: doctoring yields another connector's entry wearing your Flux — its id, service,
hosts and credentials are the shipped one's, a declaration whose name disagrees with `entry.id` is
`Error::Mismatched`, and correcting `provider` fails the index lookup with `Error::UnknownProvider`.

## Notes

- Read `docs/designs/graphql-vendors.md` first — C-110 wrote it as the record of the refusal, and it
  states which four boundaries are already solved so a future attempt does not re-derive them.
- The deeper fix for C-110's specific collision is [C-87](C-87-publish-configuration-surface.md):
  publish the configuration surface so the pack *reads* an operation's variables instead of inferring
  them from syntax. `crates/connector-pack/src/request.rs` says as much about its own scan — that it
  infers configuration from Flux *"rather than waiting for C-87"*. This story is the narrower one:
  even with the scan as it stands, an implementor should be able to see the refusal.
- Do not let this become "give implementors the whole catalogue". The index is coordinator-owned for
  a good reason ([AGENTS.md](../../AGENTS.md), whole-catalogue artifacts), and a provider story that
  regenerates it is the defect that rule exists to prevent.
