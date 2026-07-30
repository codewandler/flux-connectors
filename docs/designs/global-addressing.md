# Design: stable global addresses for providers and operations

**Status:** approved, **amended by C-49** · **Pillar:** Spec ·
**Stories:** [C-37](../stories/C-37-global-addressing.md)

> **Amendment (C-49, [provider-services.md](provider-services.md)).** This design's **middle level is
> no longer anonymous.** Its first path segment is a declared
> [`Service`](../stories/C-49-provider-services.md) — a named thing that owns the base URL, the
> description and the API version — not a bare `Operation.path` segment. Three consequences bind
> anything built on this document:
>
> 1. **`api_version` belongs to the service**, with the connector-level value as its default. A single
>    connector-level version cannot describe AWS, which dates `s3` at `2006-03-01` and
>    `bedrock-runtime` at `2023-09-30`.
> 2. **A `default` service is reserved, implicit and elided.** An operation that names no service
>    belongs to `default`, and `default` is **never rendered**: `com.freshdesk.api:v2`, not
>    `com.freshdesk.api/default:v2`. `default` must never reach a published address.
> 3. **`Operation.path: Vec<String>` is not the shape to build.** C-49 landed the first segment as
>    `Operation.service: String`. C-37's remaining segments, if it still wants them, append *below* the
>    service — and doing so makes parsing ambiguous, because `com.freshdesk.api/tickets:v2` could be
>    the `tickets` service or a tail segment under an elided `default`. C-37 must pick one of the two
>    resolutions provider-services.md records (parse against the connector's declared service set, or
>    forbid a tail on `default`) before it can claim the round-trip law. **The grammar implemented today
>    refuses a gid with more than one middle segment** rather than guessing.
>
> Where this document says "a versioned resource group", read "a service". Where it says
> `Operation.path`, read `Operation.service` plus whatever C-37 adds below it.

## Why

Every identifier in this repo today is **local and untyped**. `Connector.id` is `"zendesk"`;
`Operation.id` is `"zendesk-ticket-show"`. Both are flat strings, unique only within this repo, and
carrying no vendor identity, no API surface, and no version.

That is already costing us:

- **Nothing distinguishes two connectors for the same vendor.** Zendesk Support and Zendesk Chat are
  different APIs; `id = "zendesk"` can only name one.
- **The vendor's API version is invisible.** `providers/babelforce.toml` targets babelforce's manager
  API, but nothing records *which version* — so a v1→v2 migration would rewrite operations in place
  rather than letting both coexist.
- **Operations are not addressable from outside.** Generated docs (C-31), the lockfile (C-7) and any
  proxy routing (C-35) all need a stable name for "this operation". `zendesk-ticket-show` is a
  symbol, not an address.

**The hinge:** this cannot be fixed by making `Operation.id` richer. C-8 established that flux's
`decl_name` grammar admits only ASCII alphanumerics, `_` and `-`
(`../flux/crates/flux-lang/src/parser.rs`), so a dotted or slashed identifier is *structurally
impossible* as a Flux declaration name. The scheme therefore needs **two identifiers with a
deterministic relationship**: a global address, and the declarable local symbol.

## Approach

### Three levels

```
pid   com.zendesk.api                            the provider
gid   com.zendesk.api/support/tickets:v2         a versioned resource group
oip   com.zendesk.api/support/tickets:v2#show    one operation
```

```
pid  := <authority>
gid  := <authority> "/" <segment> ("/" <segment>)* ":" <api-version>
oip  := <gid> "#" <operation>
```

Each separator carries exactly one meaning — `/` hierarchy, `:` version, `#` operation — mirroring
URI path-then-fragment syntax. The middle level is not an accident of the grammar: a **gid is
independently useful**, because `com.babelforce.api/manager/calls:v1` names a resource group that
docs can section by and a registry could list.

```
com.zendesk.api/support/tickets:v2#show
com.zendesk.api/support/tickets:v2#comment-add
com.babelforce.api/manager/calls:v1#list
com.babelforce.api/manager/agents:v1#status-update
com.freshdesk.api/tickets:v2#create
```

Freshdesk has **one** path segment where the others have two. Variable depth is exactly why hierarchy
uses `/` rather than positional fields — a positional scheme would need an empty slot.

### Structured fields, rendered — not an authored string

`Connector` gains `authority` and `api_version`; `Operation` gains `path: Vec<String>`,
`operation: String`, and an `api_version` override (babelforce exposes manager/admin/agent surfaces
that may version independently). The oip string is *rendered* from them.

Each part is then validated separately, a typo in a segment cannot masquerade as a valid address, and
grouping or filtering by scope needs no parser at the call site.

New fields go **inside** `HashDomain::of` — they are part of a connector's compiled meaning, unlike
provenance, which C-7 deliberately excluded.

### The version is the vendor's, not ours

So the oip stays stable across our regenerations and changes only when the vendor versions their API.
Our own connector version already lives in `connectors.lock` alongside the generator (C-7). Mixing
them would churn every address on every release of ours and break every external reference.

### `Operation.id` is untouched

It remains the declarable Flux symbol and the LLM tool name. The four goldens in
`crates/connector-flux/tests/golden/` and every provider TOML already pin those symbols; rewriting
them would be a large diff for no gain. **C-23 remains the *local* half** — how the symbol is spelled,
that it is declarable, stable and collision-checked. This design is the *global* half. They are
complements, not competitors.

### Stability contract

> An oip, once published, is never reused for a different operation. Renaming an operation mints a
> new oip and deprecates the old one.

Lands in `AGENTS.md` beside the auth conventions.

## Alternatives considered

- **One richer `Operation.id`.** Impossible: flux cannot declare it. This is settled fact, not
  preference.
- **Positional colon-separated fields** (`com.zendesk.api/support:tickets:show:v2`). Closer to the
  original sketch, but freshdesk has no scope segment, so the arity varies or a slot sits empty, and
  prefix-matching a group becomes awkward.
- **Path hierarchy with the operation as a final segment**
  (`com.zendesk.api/support/tickets/show:v2`). Fewer separator kinds and shell-friendlier, but it
  loses the group as an addressable thing — and the group address is what makes docs sectioning and a
  future registry natural.
- **An authored oip string per operation.** Minimal schema, but a typo'd segment is a valid-looking
  id, and every consumer needs a parser.

## Risks & open questions

- **`#` is awkward in shells, URLs and TOML keys.** Accepted deliberately. Three mitigations must
  hold: an oip is never a TOML *key* (rendered, not authored); generated docs always quote it in
  shell examples; proxy routes address by path segments rather than embedding a raw oip in a URL.
- **Choosing an authority commits us.** `com.zendesk.api` is our naming of someone else's API. If a
  vendor later publishes their own identifier scheme, ours will not match. Reverse-DNS at least makes
  the collision space obvious.
- **Scope segments are a judgement call.** Is Zendesk's `support` a scope, or is `tickets` enough?
  Getting it wrong is cheap now and expensive after publication, because the stability contract binds.
- **Nothing enforces the contract across releases yet** — an oip could silently change between builds.
  `flux-connectors diff` reporting a changed oip as a breaking change is the natural follow-up, and
  belongs with C-23's rename detection.

## Acceptance / done

See [C-37](../stories/C-37-global-addressing.md). In short: all three levels render, parse and
round-trip; collisions are loud; the fields are in the hash domain; manifests and the lockfile carry
the addresses; all three providers declare them and still build.
