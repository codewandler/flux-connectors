# Design: what a member receives and what it returns

**Status:** proposed · **Pillar:** Codegen · **Stories:**
[C-124](../stories/C-124-member-io-schemas-epic.md) … C-128

## Why

Every member with inputs should state, in JSON Schema, what it accepts and what it gives back. That
is what makes a catalogue machine-usable: a UI can render a form, a flow editor can type a wire, and
`ToolSpec` can be projected without guesswork.

**Measured against the shipped catalogue rather than assumed** (97 operations, `web/public/catalog.json`):

| | coverage |
|---|---|
| operations carrying `parameters` | **92 / 97** |
| operations carrying `response_schema` | **16 / 97 (16%)** |
| operations carrying `body_schema` | 2 |
| events / channels / graphs in the catalogue at all | **0** |

So this is not one problem. It is three, and they need different treatment.

## 1 · Input: the data exists but is never composed

An operation carries per-parameter schemas (`Param::schema`, `crates/connector-spec/src/ir.rs:149`)
split across path / query / body / header, plus an optional whole-body `body_schema` (`:182`). There
is **no single JSON Schema saying "this is what the operation receives"**.

Every consumer therefore re-derives it, and each will disagree about the corners: whether a path
parameter belongs in the same object as a body field, what `required` means, how `body_schema`
merges with named body params. `ir.rs:179` already records that "assembled from named fields" and
"the body *is* this schema" are two answers to one question with no stated merge rule.

**One composed `input_schema`, derived and never authored**, settles it in one place. This is
mechanical work with a clear right answer, and it has an immediate consumer: `ToolSpec.input_schema`
is **required**, not optional (`crates/flux-spec/src/lib.rs`), so [C-114](../stories/C-114-tool-spec-projection.md)
needs exactly this. Composing it once here is strictly better than the pack inventing its own.

## 2 · Output: the trap, and the most important thing in this document

`response_schema` (`ir.rs:435`) describes **the vendor's JSON response body**. It is tempting to
publish it as "the operation's output schema". **That would be false at the flux boundary.**

`http.request` returns **one flat string** — `HTTP {status}\n{headers}\n{body}` — the constraint
`crates/connector-flux/src/op.rs` already records, and the reason error-envelope pointers live in
prose rather than in code there. So today:

> A consumer reading a published `output_schema` of `{"data": {"id": …}}` and writing `.data.id`
> against the emitted op gets **`null` on every call**, forever, with no error.

That is a documentation bug that produces silently broken flows. So the two things must not share a
name:

- **`response_schema`** — what the *vendor* sends. Documentation and UI value. Honest today.
- **the effective output** — what a *caller* actually receives. `String` today, for every operation
  without exception.

Publishing both, distinctly labelled, is truthful. Publishing one as the other is not.

### What makes a real `output_schema` possible

The [Tool pack](connector-tool-pack.md) changes this, and is the reason to do the work now rather
than deferring it. A `Tool` is Rust: it can parse the vendor's body and return structured content,
so `ToolSpec.output_schema` can finally be *true* rather than aspirational — while the composite
`.flux` path keeps returning a string until `http.request` returns a record.

That means the effective output is **per surface**, not per operation, and the catalogue has to say
which surface it is describing. C-127 owns that.

## 3 · Events, channels and graphs reach no artifact at all

The IR already has the fields — `inbound.rs:225` and `graph.rs:101` both carry an optional schema —
but nothing publishes them, so a host cannot read what a connector's inbound surface accepts.

[C-83](../stories/C-83-channel-binding-codegen.md) is the prerequisite: it publishes events and
bindings into the manifest and `catalog.json`. This design adds the **in/out dimension** on top of
it rather than duplicating it, and extends the same treatment to graphs.

The direction of each is worth stating, because it is not symmetric with an operation:

| member | "in" | "out" |
|---|---|---|
| operation | caller's arguments | vendor's response / effective result |
| event | the vendor's inbound payload | — (an event returns nothing) |
| channel binding | the inbound payload, after the payload map | the **reply operation's** input |
| graph | declared `inputs` ports | the declared `output` port |

A channel's "out" being another member's "in" is the composition C-82 already recorded, and it is
why bindings should reuse the operation's composed input schema rather than restating it.

## The rule that keeps this honest

**Every schema is derived or authored, never guessed.** An operation with no declared response shape
publishes *absence*, not `{}` or `{"type": "object"}` — a permissive placeholder is
indistinguishable from a real declaration and is worse than nothing, because it defeats the coverage
measurement above.

And coverage must be **measured and non-decreasing**: a test that reports the current figure and
fails if it drops. 16% is a starting point that can only be improved deliberately if something
watches it.

## Out of scope

- **Inferring a response schema by calling the vendor.** Generation is offline; that rule is absolute.
- **Validating a live response against its schema at runtime.** A different concern, and one that
  belongs to whoever executes.
- **A schema dialect migration.** Whatever the repo emits today stays; this is about coverage and
  composition, not about JSON Schema drafts.
