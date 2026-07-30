# flux-connectors — roadmap & status

The big picture: what's delivered, what's next, and the epics that group related stories. The
operational detail lives on the [board](stories/README.md) (generated from story frontmatter); this
document is the hand-written narrative around it.

## Status

_As of 2026-07-30:_ the repository has just been scaffolded. Nothing is implemented yet — the
backlog, the two design records, and the Cargo workspace are the whole of it. The single epic,
**connectors-v1**, carries every story. The one external dependency is a change to `../flux`
described in [designs/auth-seam.md](designs/auth-seam.md); it is on the critical path and should be
designed and filed against flux's board before the codegen work finishes.

## Delivered

- _Nothing yet._ Itemized history lands in [CHANGELOG.md](../CHANGELOG.md) as stories close.

## Next

The ranked, actionable form is the **Next** list on the [board](stories/README.md). In short:
scaffold the workspace, design the auth seam early (longest lead time), build the spec crate, then
the codegen crate, then the CLI — and finish with two providers proven end-to-end against a live
flux.

## Epics

An **epic** is a themed group of stories with a shared design doc. Stories join an epic via the
`epic: <slug>` frontmatter field, where `<slug>` matches a design doc at `docs/designs/<slug>.md`.

### Connectors v1 — spec to Flux

Prove the whole thesis on two real providers: a provider TOML plus a vendored vendor spec compiles
into a `.flux` module that flux loads as ops and exposes as LLM tools, with credentials resolved by
the host and never present in any artifact. Design:
[designs/connectors-v1.md](designs/connectors-v1.md); the pipeline itself is
[designs/connector-pipeline.md](designs/connector-pipeline.md).

**Done looks like:** `flux-connectors build && flux-connectors install`, then a `flux` session lists
`zendesk.ticket.show` and `anthropic.messages.create` among its ops and calls one successfully
against the live API.

The two providers are chosen to exercise different halves of the pipeline:

- **anthropic** — spec-driven with raw-header auth. Proves ingest → IR → codegen → registered op with
  no auth blocker in the way.
- **zendesk** — Basic auth and heavy patching. Proves the overlay layer, forces the auth seam, and
  tests the plugin-replacement claim directly against flux's `plugins/zendesk` (687 lines of Rust,
  reduced to one TOML).
