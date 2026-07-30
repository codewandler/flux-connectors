# Design: the flow graph — connector members composed into one Flux op

**Status:** accepted (the IR landed) · **Pillar:** Spec (+ Codegen, Bridge) ·
**Epic:** `flow-graph` · **Stories:** C-94 … C-98

## Why

Four waves built this vocabulary without naming it:

| already in the IR | graph role |
|---|---|
| `Operation` | a call node |
| `EventDecl` | a source node |
| `ChannelBinding` | *"the composition of the two"* — its own module doc |
| `Oip` (`com.slack.api:v1#name`) | a global node id, addressing **all three** member kinds |
| the dotted `wire` grammar | an edge |
| `Reply::result` | one edge whose source is the flow's own computation |

What was missing is the **graph**: a way to say *this operation, then that gate, then that operation*,
and a schema an editor can render. `connectors` connects things; this is the connecting.

## This is not the second language the north star forbids

The objection writes itself — *"No homegrown DSL… We never invent a second little language to sit in
front of it."* This repository has ruled on that four times, and the rulings are consistent:

| ruled on | outcome |
|---|---|
| a template DSL (`:params.req_id`, `{{context.api_host}}`) | **rejected** |
| JSONPath as a second path grammar | **rejected** |
| a vendor's `POST /expressions/evaluate` as an op | **rejected** |
| `[inbound]` events — a whole new declarative section | **accepted** |
| channel bindings — payload maps, replies, transports | **accepted** |
| `[[config]]` — labels, formats, bindings | **accepted** |

**Every rejection was an *expression* language. Every acceptance was declarative structure that
compiles to Flux.** So the line is exact, and it is the one thing this design must never cross: no
node carries a user-typed formula.

`Condition` is a closed comparison — a port reference, one of seven operators, a literal — and the
Flux expression is *generated from it*. That is the entire difference between this and action-proxy's
`retry: ':params.disable_retry ? -1 : 2'`.

`NodeKind::free_text` is the tripwire. It destructures every variant exhaustively, so a field added
later fails to compile until somebody classifies it as a `Reference`, a `Path`, a `Template`, `Prose`,
`Data` or a `Schedule`. **There is deliberately no `Formula` role**, and needing one is the signal to
stop rather than to add a variant.

## A view onto Flux, not a layer over it

`flux_lang::ast::Node` has **43 kinds**; this repository constructs **nine**. Every node here already
exists in the language and is merely unexposed:

```text
Throttle  → Node::Throttle {name, max, window_ms}     Gate     → Node::When
Approval  → Node::Confirm  {message, risk}            Select   → Node::Jq
Retry     → Node::Retry    {max, backoff, delay_ms}   Template → Node::Fmt
Object    → Node::Obj      Literal → Node::Lit
```

That is the strongest available argument that this is a projection rather than a language. Still
unexposed and filed for C-98: `Parallel`, `Race`, `Match`, `Route`, `Fallback`, `Each`, `Saga`,
`Timeout`, `Budget`.

## The boundary: two node kinds are not Flux nodes at all

**flux lifts only `op` declarations** from `~/.flux/flows`. `channel` and `trigger` are Program
members an operator writes. So the node kinds that wake a flow — `Trigger`, `Schedule`, `Endpoint` —
are the graph's **boundary**: they declare what wakes it, become the emitted op's parameters, and are
**emitted nowhere**. The operator writes the two-line program that binds them, exactly as C-63 does
for the poll transport.

A boundary node therefore takes no inputs and sits in no region — nothing inside the flow can feed
what wakes it, and a boundary cannot be conditional or retried.

## Control flow must nest; data flow need not

Flux has no `goto`. A statement may read any bound symbol, so **data convergence is free** — a diamond
is a legal graph. But a branch is a nested block, so **control must nest**. Four rules follow, and the
third is the one with teeth:

1. **No cycles.** A cyclic graph has no lowering at all; an iteration is a bounded loop node, never an
   edge pointing backwards.
2. **No edge leaves a region except through a port the region declares.** A value may enter freely —
   an inner statement reading an outer symbol is fine — but the escaping direction is where a symbol
   might not be bound when the block closes. A region's output ports are the phi node Flux does not
   have, made explicit; they are `Reply::result` generalised.
3. **A `Gate` may declare no outputs.** It lowers to `when`, which has no else branch here, so a
   symbol bound inside is *unbound* on the false path and reading it afterwards fails at runtime —
   long after the build passed. Exporting a value out of a conditional needs a branch with a default
   (C-98's `Match`).
4. A region that always runs its body or fails — `Retry`, `Throttle`, `Approval` — may export freely.
   The contrast is what shows rule 3 is about Flux's semantics rather than a blanket ban.

## Edges are symbols the compiler owns

`Edge { from: PortRef, to: PortRef }`. Lowering generates one `$symbol` per edge, satisfying flux's
identifier grammar and avoiding its ~70 reserved words. **An author never sees or names one** — which
is what makes action-proxy's silent `$emit` shadowing unrepresentable. (Its `call.update.yml` binds
`base_fields` twice, invisibly.)

Node ids are **author-stable**, deliberately unlike `flux_lang::ast::NodeId`, which is a positional
index into the body and is invalidated by any edit. A saved graph must survive re-ordering.

## What action-proxy proves is needed

It is the cautionary tale, but **106 of its files use multi-step `do:` pipelines** — the need is not in
doubt. What to serve, and what to avoid:

| its shape | here |
|---|---|
| three expression languages by string prefix, re-entrant | no expression field; one closed comparison |
| the executable interior is `{type: object}` — *"the worst split, because it looks validated"* | the interior is exactly what this types |
| `$emit` into a flat mutable scope, shadowing silently | edges are generated symbols |
| if/else as two negated `$if` steps | one node with a condition |
| `variables.yml` — hand-maintained, untyped *"what will exist after this node runs"* | **typed output ports**; this is the whole argument for them |
| `required_expressions:` — advisory ambient dependencies | an incoming edge |

## Neighbours

- flux's **Railflux** (L-95) projects an *existing* `DraftAst` to ASCII. This authors a graph over
  *catalogued connector members* — one level up. Different jobs; do not merge them.
- flux shipped **D-139** (`Diagnostic.node_path` = `"body[2].then[0]"`) precisely because
  *"ai-agent-platform's `NodeMap` keys analyzer findings back to graph-canvas nodes"*. C-96 reuses
  that seam rather than parsing message text.

## The blocker to state loudly

**`http.request` returns one flat string** — `HTTP {status}\n{headers}\n{body}`. `op.rs` records why
error-envelope pointers land in *prose* rather than code: a pointer applied to that string resolves to
`null` on every response, and splitting it needs an `expr` escape that is not a fixed point of flux's
formatter, so the module stops round-tripping.

**So a `Select` node wired to an `Operation` output is exactly the case that cannot lower yet.** The
IR may declare the port — declaring is this repository's job — but C-95's lowering must refuse it
until `http.request` returns a record, and that refusal belongs in its acceptance. A flow editor
drawing `response.error.message` as a draggable port would be drawing a wire that cannot be connected.
This is the most likely way the whole idea disappoints in practice, so it is written down first.

## What this does not settle

- **Lowering** — C-95, through `flux_lang::dsl`. Owns symbol generation and region nesting.
- **Richer node kinds** — C-98. All exist in flux-lang; this is exposure, not invention.
- **The operator program** — C-97, a documented pattern, emitted nowhere.
- **Cross-connector graphs.** Bindings are intra-connector and intra-service; spanning two vendors is
  a real question and a separate one.
- **A visual editor.** This ships the schema one consumes.

## Alternatives considered

- **A YAML flow config a runtime walks.** action-proxy, with better types — and it would collide with
  both the north star (*"behavior back into config the runtime reads directly"*) and the `no runtime`
  non-goal.
- **Compile to a whole Program** (channel + trigger + journey). No artifact home today, since flux
  lifts only `op`. Filed behind C-97 rather than blocking the useful half.
- **A general DAG with arbitrary edges.** Flux has no `goto`; an irreducible graph cannot be lowered,
  and accepting one would mean guessing.
- **A new `connector-graph` crate.** The existing split already covers it — `connector-spec` owns pure
  IR and never touches the network, `connector-flux` owns lowering and never string-templates.
