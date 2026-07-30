---
id: C-95
title: Lower a flow graph to a composite Flux op
pillar: Codegen
status: blocked
priority: 3
design: docs/designs/flow-graph.md
epic: flow-graph
areas: [connector-flux]
note: "owns symbol generation and region nesting. MUST refuse a Select wired to an Operation output until http.request returns a record — today the response is one flat string"
---

# Lower a flow graph to a composite Flux op

## Goal
Turn a graph into real Flux, through `flux_lang`'s AST — the payoff for targeting a language instead
of interpreting config.

## Acceptance
- [ ] `connector-flux` gains a graph lowering producing a `CompositeOpDecl`, built through
      `flux_lang::dsl` or `ast` and formatted by flux's own formatter. **Never string templates** —
      the rule this crate exists to enforce.
- [ ] **Symbol generation is the lowering's own.** One `$symbol` per edge, satisfying
      `is_identifier` and avoiding flux's ~70 reserved words, stable across rebuilds so a regenerated
      module does not churn. An author never sees one.
- [ ] Regions nest correctly: a region's nodes lower into its block, and a declared output port
      becomes the block's `-> $bind` where the kind supports one.
- [ ] **The emitted module parses and analyzes**, the C-11 gate. Format-then-reparse is the cheap
      total round-trip check.
- [ ] **A `Select` wired to an `Operation` output is refused**, with the reason: `http.request`
      returns `HTTP {status}\n{headers}\n{body}` as one flat string, so a path applied to it resolves
      to `null` on every response. Splitting it needs an `expr` escape that is not a fixed point of
      flux's formatter. This is a refusal, not a degradation — emitting a selector that always yields
      null is precisely the plausible-but-wrong output `AGENTS.md` forbids.
- [ ] Golden `.flux` files for the worked example and one region-per-kind.
- [ ] **Shares C-12's lowering.** C-12 turns a declared quirk into `retry`/`throttle`/a bounded loop;
      a `Retry` or `Throttle` node needs the identical construction. Two code paths emitting different
      Flux for the same intent is the failure to avoid.

## Progress
- Not started. The IR landed 2026-07-30 with [C-94](C-94-flow-graph-epic.md).

## Notes
- `flux_lang::dsl` is a Rust builder that compiles straight to the AST and is currently unused here —
  the obvious lowering target.
- Constraints found while reading flux: a call argument takes values, never a bare `fmt` (bind first,
  then pass `$name`); `Obj`/`List` leaves must be pure value nodes; `await` and `checkpoint` are
  top-level only; a composite op may not recurse.

## Progress

- **Integration attempt reverted (coordinator).** The branch was cut before the flux-lang 0.37 → 0.39 upgrade landed on `main`. Merged, gate went red with **8 of 21** `graph_emitter` tests failing, and `UPDATE_GOLDEN=1` did not clear them — only `graph-message-autoreply.flux` moved, and `graph-nightly-sweep.flux` did not re-record, which suggests its emit now fails rather than merely differs. Three failures are structural rather than textual, so this is not a spelling migration.

  The merge was reverted with `git revert -m 1`; `impl/C-95` is intact. Sent back to the implementor to merge `main`, work the failures, and re-take its base proof. Prime suspect, which the implementor itself named: `retry … -> $bind` plus a trailing bare symbol reference may no longer be a formatter fixed point under 0.39, in which case `check_canonical` is correctly refusing and the region-output lowering needs a different shape.
