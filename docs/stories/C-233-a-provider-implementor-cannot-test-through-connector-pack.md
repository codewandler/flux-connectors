---
id: C-233
title: "A provider implementor cannot exercise their own connector through `connector-pack`, so 'does it compose a request at all' is unanswerable until integration"
pillar: Build
status: ready
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

- [ ] **Failing-first test:** a provider implementor, working only in their own worktree with no
      whole-catalogue artifact regenerated, can execute an operation through `connector-pack` and
      observe it refuse. Name it. Demonstrate it against C-110's withdrawn Linear documents, which
      are preserved as a fixture in `crates/connector-flux/tests/linear_connector.rs` and are a known
      positive.
- [ ] The route does **not** require regenerating the index. If the answer is a builder or a test-only
      constructor for `catalog::Operation`, say what `#[non_exhaustive]` was protecting and why the
      new route does not reopen it.
- [ ] The check runs inside the **scoped provider gate** `AGENTS.md` documents, so an implementor
      gets the answer without being told to look. A capability nobody is pointed at is a capability
      nobody uses.
- [ ] `AGENTS.md` records it beside the eight-red-tests procedure, which is where a provider
      implementor is already reading.
- [ ] The configuration used is what the provider **declares**, including the empty case — see
      [C-232](C-232-the-request-test-fabricates-the-values-that-hide-a-refusal.md), which is the same
      failure from the catalogue side. These two should be designed together; fixing either alone
      leaves the other's hole open.

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
