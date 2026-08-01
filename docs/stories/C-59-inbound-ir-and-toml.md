---
id: C-59
title: An `[inbound]` section in the provider TOML and the IR
pillar: Spec
status: ready
priority: 2
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-spec]
note: "SUPERSEDED IN SPELLING — no `[inbound]` section shipped; the capability landed as C-82's `[[events]]`/`[[channels]]` member kinds. Needs a close-or-rescope decision, not an implementor; see Progress for the bullet-by-bullet measurement"
---

# An `[inbound]` section in the provider TOML and the IR

## Goal

Extend the spec front-end so a provider can declare what it sends us, with the same discipline the
outbound side already has: typed, hermetic, and provenance-tracked.

## Acceptance

- [ ] `[inbound]` parses into a new IR module: `transport`, `verification`, `discriminator`,
      `delivery_id`, and `[[inbound.event]]` entries with `name`, optional `when`, optional `schema`.
- [ ] Failing-first test `inbound_toml_round_trips_to_ir` over a fixture covering all four verification
      shapes in the design's table.
- [ ] `secret`/`secret_ref` in a verification block is a **credential name**; a literal-looking value is
      a **parse error**, not a warning (invariant 3 — the secret must never be able to enter an artifact).
- [ ] A `{timestamp}` in the `signed` template without a `tolerance` is a parse error (invariant 5).
- [ ] Event payload schema refs resolve against the vendored spec cache, and an unresolvable ref fails
      the build rather than degrading to untyped.
- [ ] Inbound facts participate in provenance and the lockfile, so drift detection covers them.

## Progress

- **"(not started)" was false.** Corrected 2026-08-01 against the tree. **This story's spelling never
  shipped and is not what exists**: there is no `[inbound]` section and no `[[inbound.event]]`. The
  capability landed instead as the `[[events]]` and `[[channels]]` **member kinds** under
  [C-82](C-82-channel-bindings-epic.md), which `AGENTS.md` §Member contract now states as the
  authority. So this story was **superseded in spelling and largely satisfied in substance**.
- Bullet by bullet, measured rather than assumed:

  | acceptance bullet | measured state |
  |---|---|
  | `[inbound]` parses into a new IR module | **superseded** — `inbound.rs` ships 13 types under the member-kind model |
  | failing-first `inbound_toml_round_trips_to_ir` | **no test of that name**; the substance is in `tests/channel_bindings.rs` and `tests/verification_conformance.rs` |
  | `{timestamp}` without `tolerance` is a parse error | **implemented** — `inbound.rs:561` requires `HmacSpec::tolerance` exactly when `{timestamp}` is present |
  | `secret` is a credential name, a literal is a parse error | **not verified this session** — `HmacSpec::secret: String` exists (`inbound.rs:226`); whether the loader refuses a literal was not checked |
  | event payload schema refs resolve against the vendored spec cache | **not what shipped** — `EventDecl::schema` is an inline `Option<JsonSchema>` (`inbound.rs:297`), not a resolved ref |
  | inbound facts participate in provenance and the lockfile | **no evidence found**; note events emit nothing into a module by design, so this may be moot rather than open |

- **What this story needs is a decision, not an implementor.** Three of the six bullets are satisfied
  under a different design, one is unverified, and two describe mechanisms that were not built. Close
  it as superseded, or re-scope it to the two genuinely-open bullets. Left `ready` rather than
  silently closed, because closing it on this evidence would itself be an unverified claim.
