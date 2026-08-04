# Design: the connectors datasource — the catalogue, queryable from a session

**Status:** proposed · **Pillar:** Bridge · **Stories:**
[C-137](../stories/C-137-connectors-datasource-epic.md) … C-140 · **Amended 2026-08-04** per
flux-roadmap Decision 0006 (rule 9): the catalogue binds as an **indexed** `DatasourceBackend`,
not `LiveDatasource`

> Read in `/home/timo/projects/flux`. Re-grep by symbol; line numbers move.

## Scope — this is the datasource *about* connectors

This design is the **catalogue-about-connectors** datasource: the compiled-in catalogue this
repository publishes, made queryable from a session. It is distinct from **vendor-data datasource
definitions** — what a *vendor's* data surface knows and how to read it — which Decision 0006
rule 5 also places in this repository and which
[vendor-datasource-declarations.md](vendor-datasource-declarations.md) owns as the `[[datasources]]`
connector surface. One reads about connectors, offline and in-process; the other declares vendor
reads that Exchange executes. Conflating them is the misread the decision exists to prevent.

## Why

A flux session has no way to ask **"which connector can do this?"**. The catalogue exists, it is
published, and it is unreachable from inside a running flow — so an agent either has every connector
operation registered as a tool, or it has none.

That is the problem this solves, and it is more pressing than it looks because of the epic next door.

### The scaling argument, which is the real reason to build this

[C-113](../stories/C-113-tool-pack-epic.md)'s Tool pack registers **one tool per operation**. That is
97 operations today across 17 providers, and the provider fleet stories will multiply it. Every one
of those is model-facing surface: schema in the context window, a name to disambiguate, a chance to
pick wrong.

A datasource is **a fixed handful of operations regardless of catalogue size**. Search, get, list,
relation, batch-get, sources — six, whether the catalogue holds 97 operations or 970.

So the two are **complementary, not competing**: *discover* through the datasource, *invoke* through
the pack. A host that registers the datasource plus a small selected pack gets the whole catalogue's
knowledge at a fraction of the context cost.

## The seam already exists — and it is the indexed one

- `flux_capabilities::DatasourceBackend` — the trait to implement
  (`crates/flux-capabilities/src/datasource/mod.rs:108`, read 2026-08-04): the six retrieval verbs
  `search` / `get` / `list` / `relation` / `batch_get` / `sources`, plus mutating index methods
  (`upsert`, `clear`, `delete_source`, `delete`) this read-only backend refuses (see below).
- `try_register_datasource_ops(registry, backend)` / `datasource_tools(backend)`
  (`crates/flux-capabilities/src/datasource/ops.rs`) — installs the six-op retrieval pack into the
  same `ToolRegistry` a host already hands the Tool pack's declarations, so discovery and
  invocation are configured in one place.

And the input/output vocabulary is already typed in `flux-datasource`: `Source`, `Record`, `Link`,
`SchemaField`, `EntitySchema`, `Declaration`, `SearchInput`, `Match`, `SourceSummary`, `GetInput`,
`ListInput`, `RelationInput`, `BatchGetInput`.

Nothing here is invented. This design is an *implementation* of an existing flux capability against a
dataset this repository already publishes.

### Why indexed, not live — Decision 0006 rule 9

Earlier revisions of this design and its stories named `LiveDatasource` and
`ClientBuilder::try_with_live_datasource`. That was the wrong trait, and the stories' own
acceptance already proved it: `LiveDatasource` is flux's governed read-through to a system of
record — `schema`/`list`/`get` with opaque cursors, projected as two generated tools
(`<domain>.list`, `<domain>.get`), and **no search, no relation, no batch-get**. C-137's search
acceptance, C-138's `RelationInput` traversal and C-140's whole charter cannot be satisfied by that
method set. A live binding buys nothing for an in-process compiled dataset — there is no remote
system of record to read through — and costs the search surface. Decision 0006 rule 9 therefore
fixes the catalogue datasource as **indexed mode**, and C-137…C-140 are amended before any
dispatch.

Two consequences worth stating:

- **The mutating trait methods are refused, typed.** `DatasourceBackend` is an index trait, so it
  carries `upsert`/`clear`/`delete_source`/`delete`. The catalogue is generated from
  `providers/*.toml` and is a fixed point of a build; a backend that could mutate it would give the
  generated tree two writers. This backend returns a typed refusal from every mutating method —
  never a silent no-op, which would report success for a write that did not happen.
- **Registration stays an owner decision.** Per Decision 0006 rule 3, the SDK seam remains the
  embedder path, and Flux-Lang's `datasource` declaration gains a kind binding the compiled-in
  connectors catalogue — that half is flux's, not this repository's.

## The catalogue is already shaped like a datasource

That is the pleasing part, and it is not a coincidence — it is what the addressing work bought:

| datasource concept | what the catalogue already has |
|---|---|
| record id | the **`oip`** — `authority[/service]:version#member` ([C-37](../stories/C-37-global-addressing.md)) |
| entity kind | provider · service · operation · event · channel binding · config field |
| link | provider→service, service→member, binding→**reply operation**, operation→credential, operation→host |
| entity schema | the IR's own declared types, already published to `catalog.json` |

A binding's link to its reply operation is the composition [C-82](../stories/C-82-channel-bindings-epic.md)
already recorded. `Link` is the type that makes it traversable rather than merely documented.

## Offline, in-process — and the thing this must not become

**"A connectors API" must not be an HTTP service.** `vision.md`:

> **A runtime.** This repo compiles; flux executes. flux-connectors ships no server, no daemon, and
> no request path of its own.

A server over the catalogue is [connectors-proxy.md](connectors-proxy.md)'s charter question again,
and [C-34](../stories/C-34-proxy-charter-decision.md) already gates that. This datasource is a **library
backend reading a committed dataset in-process** — no socket, no daemon, no network.

The source is the `catalog` crate: compiled in, deterministic, and offline by construction. Reading
`catalog.json` from disk at runtime is the alternative and is worse — it introduces a path that can
be missing, stale, or edited, for no benefit a rebuild does not already give.

**Consequence to state rather than discover:** a compiled-in catalogue is exactly as fresh as the
binary. That is correct for this repo's model — the whole point is that a connector is compiled — but
a host expecting live vendor data will be surprised. Say so where the surprise would happen.

## What "search them" has to mean

A search that returns the wrong connector confidently is worse than no search, because the caller
acts on it. So the search story is separate ([C-140](../stories/C-140-datasource-search.md)) and its
acceptance is about *quality*, not about wiring:

- Search over the fields a caller actually reasons in: vendor, description, operation id, and — once
  [C-119](../stories/C-119-provider-roles-epic.md) lands — **role**. "Find me a ticketing provider" is
  a role query, and it is the query that makes the whole roles epic pay off.
- Rank deterministically. A tie broken by hash order makes the same question answer differently
  between builds, and the catalogue's fixed-point discipline should extend to its queries.
- Return **why** a record matched. A `Match` a caller cannot explain is a `Match` a caller cannot
  check.

## Out of scope

- **A server, a daemon or an HTTP API.** See above; that is C-34's decision, not this epic's.
- **Live vendor data.** The datasource answers about *the catalogue*, never by calling a vendor.
  "Does this Zendesk ticket exist" is an operation; "which connector has ticket operations" is this.
  Vendor-data datasources are the `[[datasources]]` connector surface —
  [vendor-datasource-declarations.md](vendor-datasource-declarations.md) — read through Exchange,
  never through this backend.
- **Writing.** The catalogue is generated from `providers/*.toml` and is a fixed point of a build. A
  datasource that could mutate it would have two writers and no source of truth — which is why the
  trait's mutating methods return typed refusals here rather than being implemented.
- **Embeddings or semantic search.** `flux-capabilities` has an `Embedder`, and it may be worth it
  later. Ship deterministic lexical search first, and find out whether it is actually insufficient
  rather than assuming.
