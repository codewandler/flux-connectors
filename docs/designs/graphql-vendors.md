# A GraphQL vendor cannot be a connector yet, and the reason is one line in the host library

**Status:** finding. The deliverable of [C-110](../stories/C-110-provider-linear.md), which asked
whether a connector can describe a vendor with **one endpoint and a query language** and permitted a
documented refusal as a first-class answer.

**Outcome:** no `providers/linear.toml`. A complete eight-operation Linear connector was written,
emitted, and passed every check in the compiler. It could not make a single call.

---

## The short version

`connector-pack` decides an operation's configuration variables by scanning **every string literal in
the emitted body for `{…}`** — `endpoint_variables`/`scan_node` in
`crates/connector-pack/src/request.rs`. A GraphQL query document is a string literal full of braces,
and none of them is configuration.

That module's own documentation states the invariant this breaks:

> In the shipped catalogue the string literals carrying braces are of exactly two kinds, **and both
> are configuration** — the ten templated base URLs, and the pin binds C-187 added.

A GraphQL document is a third kind. The sentence stays true only because this connector was
withdrawn.

## Why it is not a small problem

Both outcomes of the misreading are wrong, and the second is worse than the first.

**Unconfigured — the production shape.** No `[[config]]` field could sensibly declare
`{ viewer { … } }`, so a tenant supplies nothing, and `Operation::build_request` refuses before
assembling anything. The refusal names a "variable" that is a fragment of GraphQL:

```
`linear-viewer` needs `endpoint.id
    name
    displayName
    email
    admin` of service `default` … and the bound configuration supplies none
```

**Configured — the document is rewritten.** `Build::substitute` replaces the brace run with the
host's value:

```json
{"query":"query Viewer a-viewer\n}\n"}
```

The `{ viewer { … } }` selection set is gone, replaced by a configuration value. This falsifies the
one property the connector existed to demonstrate — that the query is a **constant the caller may
not choose**. It is constant against the *model*, and editable by whoever supplies the tenant's
settings. That is a worse position than not shipping.

Both halves are measured, not argued, in `crates/connector-pack/src/request.rs`:
`a_graphql_document_in_a_literal_is_read_as_configuration_variables` and
`a_graphql_operation_cannot_be_called_and_is_corrupted_when_it_is`.

## The fix belongs to C-87, and the pack should not be weakened

The scan is not a mistake. It is a **stand-in**, and the module says so: it reads configuration off
the emitted Flux "rather than waiting for C-87 to publish them". The heuristic is sound for
everything that ships, and the alternative it guards against is worse — substituting over the
finished URL would reach a caller's parameter values, which is the C-193 hazard.

So the fix is not "make the pack smarter about braces", and it is certainly not "let a provider
declare `endpoint.*` fields to absorb the selection sets" — that is a connector shaped around a
defect. The fix is [C-87](../stories/C-87-configuration-codegen.md): publish the configuration
surface into the manifest and the catalogue, so the pack **reads** an operation's configuration
variables instead of **inferring** them from syntax. Once an operation states its variables, a brace
inside a vendor constant stops being anybody's business, and the two-kinds invariant stops needing to
be true.

A narrower mechanism — marking a literal opaque to the scan — is possible but should be weighed
against C-87 rather than landed ahead of it. It would put the same fact in two places, and the
emitter has no vocabulary for "opaque" that is not inventing Flux semantics.

## What the probe established, so a later attempt does not re-derive it

Four of the six things a GraphQL vendor needs are **already expressible**, three with no new
mechanism at all. Each is pinned in `crates/connector-flux/tests/linear_connector.rs`.

| # | Question | Answer |
|---|---|---|
| 1 | Does anything key an operation by its path? | **No.** Identity is `id`; `catalog::Operation` has no `path` field. Zendesk already ships three operations on one `PUT` path. |
| 2 | Does C-55's constant body field cover a query document? | **Yes**, genuinely rather than by resemblance. `constant` is a bare `schema.get("const")` — no type, length or newline rule — and the emitter keeps constants out of the signature. |
| 3 | Does a multi-line document survive the emitter? | **Yes**, as a verbatim `"""…"""` block. No provider had exercised that path. |
| 4 | Can a response schema express `data.<field>`? | **Yes**, and it is *stronger* than a REST schema: the shape under `data` is a consequence of the document pinned beside it. |

Two are not expressible, both on the safety axes. Neither would have withdrawn the connector on its
own; they are recorded because they are real and because the next attempt inherits them.

**5. `risk`, `idempotency`, and direction are authored for every operation.**
`check_write_metadata` reads the closed vendor-state direction, never the HTTP verb. GraphQL can
therefore state a query transported by `POST` as `read` and a mutation transported by the same verb
as `write`; the stable operation identity, not transport shape, carries the reviewed distinction.

The guard remains strict where it matters: an authored write may not claim `low` or `idempotent`,
while an authored read may. If GraphQL support is attempted again,
the shape is an operation declaring that its verb is a transport detail, not a loosening of the check.

**6. A failed call arrives as HTTP 200, and nothing can say so.** This is
[C-57](../stories/C-57-quirks-beyond-http-shape.md)'s exact case. Linear answers a validation error,
a permission denial and an expired key alike with `200`, a `null` `data` and an `errors` array;
flux-web hardcodes `is_error: false` for any completed request, and the emitted Flux asserts nothing
on status. **Every GraphQL failure reads as a success to anything switching on status.**

`ErrorEnvelope` cannot express it — no success predicate, and its own documentation scopes it to a
*non-2xx* body. Worse, declaring one is actively harmful here: `description()` appends *"A non-2xx
response is returned as data, not a failure…"* to the contract a model reads, which for a GraphQL
vendor points at a branch that never occurs. And there is no compensating structured data, because
`quirks` reaches **no artifact at all** — `connector-cli`'s catalogue and site emitters both write
`Quirks::default()`. So the choice was between silence and a false sentence, and the withdrawn
connector chose silence.

A third, smaller instance of the same root: cursor pagination is undeclarable, because
`Pagination::Cursor.cursor_param` names a **query parameter** and a GraphQL cursor is a body
variable. C-57's fourth acceptance item; Linear would have been the second provider to hit it after
Slack.

## Why the first attempt's gate was green

This is the part worth keeping.

`crates/connector-pack/tests/request.rs::every_shipped_operation_builds_an_absolute_request` walks
every shipped operation and asserts it builds. It did not catch this, for two independent reasons:

1. **It asserts only on the URL.** A GraphQL document lives in the body, and the URL composes fine.
2. **Its `configuration()` helper manufactures a value for every *discovered* variable.** So it
   fabricates exactly the values that hide the refusal — and, in doing so, silently produces the
   corrupted document it never inspects.

`every_shipped_configuration_variable_is_placed` (C-214) **would** have caught it at integration: a
GraphQL fragment has no request position, so every one of these variables comes back unplaced. That
check postdates C-110's first attempt, which is exactly why that attempt's gate was green. It is
pinned as part of this finding so the coincidence does not have to hold next time.

There is also a structural reason a provider implementor could not have found this: **a new
provider's operations are not in the catalogue index until the coordinator regenerates it**, and the
index is coordinator-owned. So no test a provider story can run reaches `connector-pack` with its own
connector in it. Any future provider whose shape is genuinely novel has the same blind spot, and the
only defence available today is a fixture test in the pack's own crate — which is where this
finding's tests live.
