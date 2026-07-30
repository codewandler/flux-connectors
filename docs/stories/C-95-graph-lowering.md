---
id: C-95
title: Lower a flow graph to a composite Flux op
pillar: Codegen
status: in-progress
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
- [x] `connector-flux` gains a graph lowering producing a `CompositeOpDecl`, built through
      `flux_lang::dsl` or `ast` and formatted by flux's own formatter. **Never string templates** —
      the rule this crate exists to enforce.
- [x] **Symbol generation is the lowering's own.** One `$symbol` per edge, satisfying
      `is_identifier` and avoiding flux's ~70 reserved words, stable across rebuilds so a regenerated
      module does not churn. An author never sees one.
- [x] Regions nest correctly: a region's nodes lower into its block, and a declared output port
      becomes the block's `-> $bind` where the kind supports one.
- [x] **The emitted module parses and analyzes**, the C-11 gate. Format-then-reparse is the cheap
      total round-trip check.
- [x] **A `Select` wired to an `Operation` output is refused**, with the reason: `http.request`
      returns `HTTP {status}\n{headers}\n{body}` as one flat string, so a path applied to it resolves
      to `null` on every response. Splitting it needs an `expr` escape that is not a fixed point of
      flux's formatter. This is a refusal, not a degradation — emitting a selector that always yields
      null is precisely the plausible-but-wrong output `AGENTS.md` forbids.
- [x] Golden `.flux` files for the worked example and one region-per-kind — *three* of the four
      kinds; `throttle` cannot be spelled by flux-lang 0.39 (see Progress) and is pinned as a
      refusal instead.
- [ ] **Shares C-12's lowering.** C-12 turns a declared quirk into `retry`/`throttle`/a bounded loop;
      a `Retry` or `Throttle` node needs the identical construction. Two code paths emitting different
      Flux for the same intent is the failure to avoid.

## Progress

### Round 2 — migrated to flux-lang 0.39

The first merge was reverted: the branch was cut before the 0.39 upgrade and 8 of 21 tests went red
on `main`. Merged `main` in (`--no-ff`), reverted `main`'s revert to bring the work back, and worked
the failures. **Seven of the eight had one root cause**, not eight: `nightly-sweep` stopped lowering
at all, so every test using it failed — the structural assertions were fine, they just never ran.

Three things changed, and only one was spelling:

1. **Goldens re-recorded** for 0.39's canonical source (a local binding drops the `$` sigil; a call
   takes direct named arguments). Re-recorded **only after** every structural test was green, so no
   broken emit is locked in.
2. **The reserved-word list is gone.** `Symbols::guarded` now takes a predicate and the lowering
   passes `flux_lang::ast::is_reserved_word`. Under 0.39 a bare `retry = …` reads as a statement
   keyword, so a transcribed list is not a style question — the tests now also assert
   `is_bare_symbol_name` on every generated name.
3. **A new refusal, and it is upstream.** flux-lang 0.39's two formatters disagree about how to
   spell a duration: `flux_lang::format` (the AST printer this crate emits through) writes
   `delay 500ms` / `per 1m`, while `flux_lang::format_cst::format_module` — the formatter a human
   editing the generated file runs — accepts only bare milliseconds (`delay 500`, `per 60000`) and
   returns `None` rather than re-printing the suffixed form. Both spellings parse to the *same* AST,
   so this is a defect upstream, not an ambiguity here. Proven at the merge base with flux-lang
   alone, no connector code involved.

   It bites **every `throttle`** (no window value avoids a suffix — `fmt_duration` never emits a
   bare number) and **every `retry` carrying a delay**. A `retry` without one is unaffected.
   `Error::UnspellableDuration` refuses both, because the alternatives were to ship a generated
   module flux's own formatter cannot format, or to rewrite the token after formatting — the string
   surgery on generated Flux that AGENTS.md exists to prevent. `check_canonical` had already caught
   it; the refusal just names the cause instead of reporting "not canonical".

   **This is the one thing to re-check when the flux-lang pin moves.** Delete
   `Lowering::check_durations` and `Error::UnspellableDuration`, put the throttle back into the
   region-per-kind fixture, and re-record. `throttled_sweep` in the tests is the fixture waiting for
   that day.

Consequence: the region-per-kind golden covers `retry`, `approval` and `gate` — three of four.
`throttle` is exercised only as a refusal.

### Round 1 — the lowering

Landed as `crates/connector-flux/src/graph.rs` — `emit_graph(&Connector, &Graph) -> Result<String>`,
built from `flux_lang::ast` nodes and formatted by `flux_lang::format::format_composite_op`. Nothing
calls it from the pipeline yet: the graph reaches no artifact in this story, and
`cargo run -p connector-cli -- build` still reports *nothing written*.

**What lowers.** Nine node kinds, each naming the Flux node it *is*: `Operation` → `Call`,
`Select` → `Jq`, `Template` → `Fmt`, `Object` → `Obj`, `Literal` → `Lit`, `Gate` → `When`,
`Approval` → `Confirm`, `Retry` → `Retry`, `Throttle` → `Throttle`. `Trigger`/`Schedule`/`Endpoint`
are the boundary: each becomes a parameter of the emitted op and reaches no statement.

**What it refuses**, each because the only alternative is Flux that is wrong at runtime: a `Select`
wired to an `Operation` output (*the* blocker — resolved *through* region ports, so a response
handed out of a `retry` is still a response); a cycle; an edge leaving a region other than through a
port the region declares; a region output no node inside binds, or two nodes claiming; a gate
declaring outputs; a `retry` declaring more than one output port (flux binds one result); a dotted
graph name; a composite literal (flux's own formatter re-spaces a JSON object, so the module would
stop round-tripping); and the call-shape mistakes — an input port naming no parameter of its
operation, a template placeholder naming no port, an empty record.

**Two decisions worth re-reading before building on this.**

1. *One symbol per edge, keyed on the edge's **source port**.* Every edge has a symbol, and two
   edges out of one port share it — a fan-out binds the value once rather than binding a copy. A
   literal symbol-per-edge would emit `$b = $a` copies, which is worse Flux for the same meaning.
2. *A region's output port lowers two ways, because flux gives only `retry` a result bind.* A
   `retry`'s declared port becomes the block's `-> $bind`, and the block ends in a bare reference to
   the producing statement, which is what that bind captures. `throttle` and `confirm` have no bind
   and need none — both always run their body or fail, so the port resolves straight to the symbol
   the body bound. That contrast is why "a gate exports nothing" is about Flux's semantics rather
   than a blanket ban.

**The one acceptance item left open.** "Shares C-12's lowering" cannot be satisfied yet: C-12 is
still `backlog`, so there is no quirk lowering to share with. The construction is factored into
`graph::retry_node` and `graph::throttle_node` — including the *generated* throttle bucket name,
which flux keys its token bucket by — so C-12 calls them rather than writing a second path. Tick
this item from C-12's side.

**Two gaps found and deliberately not closed.**

- `Graph::inputs` declares op parameters that **no edge can reach**: `Edge.from` is a `PortRef`
  naming a node, and a graph input is not a node. Only a boundary node's output ports are wireable
  today. The parameters are still emitted (a caller supplies them), but either `PortRef` needs a
  form that names the graph's own input or `inputs` should fold into a boundary node. Worth settling
  in C-96/C-97 before an editor draws a port nothing can connect to.
- `Compare::Exists` lowers to flux's truthiness guard (`when $x`), the same one `op.rs` uses for an
  unsupplied query filter — so a deliberate `0`, `false` or `""` reads as absent. Documented at the
  call site rather than faked; a real presence check needs something flux does not expose.

## Notes
- `flux_lang::dsl` is a Rust builder that compiles straight to the AST and is currently unused here —
  the obvious lowering target.
- Constraints found while reading flux: a call argument takes values, never a bare `fmt` (bind first,
  then pass `$name`); `Obj`/`List` leaves must be pure value nodes; `await` and `checkpoint` are
  top-level only; a composite op may not recurse.
