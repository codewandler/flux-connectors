---
id: C-185
title: "A request body cannot contain an array, so an envelope-shaped vendor cannot be addressed"
pillar: Codegen
status: in-progress
priority: 2
areas: [connector-flux, connector-spec]
note: "found by C-168, then NARROWED by C-179: a flat single-level array DOES work (front's tag_ids emits as List<String>). What is blocked is an array a wire path must DECOMPOSE across nested segments, which is what SendGrid's personalizations[].to[] needs"
---

# A request body cannot contain an array, so an envelope-shaped vendor cannot be addressed

## Goal

Let an operation declare a request body containing an array, so a vendor whose write surface is an
envelope can be addressed at all.

## What was measured

[C-168](C-168-provider-sendgrid.md) established this rather than assuming it. `BodyNode`
(`crates/connector-flux/src/op.rs`) composes a nested **object** from a dotted `wire` path.

**The original framing of this story was too broad, and C-179 corrected it by reading `body_tree`
rather than trusting the summary.** A **flat, single-level array is already expressible** — Front's
`tag_ids` emits as `List<String>` in its shipped module. What is blocked is an array that a `wire` path
must **decompose across nested segments**. That is a narrower claim and it is the accurate one.

SendGrid's `POST /v3/mail/send` requires:

```json
{ "personalizations": [ { "to": [ { "email": "…" } ] } ], "content": [ { "type": "…", "value": "…" } ] }
```

Arrays of objects containing arrays of objects. SendGrid does not accept the bare-object form. So the
operation was **excluded**, and this catalogue now ships an email provider that cannot send email.

The one mechanically-legal workaround — a single array-typed body-root parameter — was rejected on
`providers/notion.toml`'s precedent: it decomposes nothing, and it dresses an unassisted guess at the
one shape the vendor is strictest about as a typed field.

## Why this is bigger than SendGrid

An envelope is the normal shape for a bulk or batch write. Already in or near the fleet:

- **SendGrid** — `personalizations[]`, `content[]` (excluded by C-168).
- **Postmark** ([C-180](C-180-provider-postmark.md)) — batch send is an array at the body root.
- **Algolia** ([C-164](C-164-provider-algolia.md)) — `requests[]` for batched index operations.
- **Cloudflare** ([C-169](C-169-provider-cloudflare.md)) — cache purge takes `files[]`, a *flat* array
  of strings, so per C-179's correction this one is **not** blocked and was wrongly listed here.
- **Miro** ([C-183](C-183-provider-miro.md)) and **Webflow** ([C-182](C-182-provider-webflow.md)) — bulk item writes.

Each will hit this independently and each will have to decide again whether to exclude the operation.
That is the argument for fixing the mechanism rather than recording the gap five more times.

## Acceptance

- [x] A body field can declare an array, including an array **of objects**, and including an array
      nested inside an object. Decide how deep the spelling goes and **record what it refuses** — an
      unbounded recursive body model is how a connector ends up shipping an untyped blob, which is the
      outcome C-107 and C-168 both refused.
      → `wire = "personalizations[0].to[0].email"`; `BodyNode::Elements`
      (`crates/connector-flux/src/op.rs`), lowered to `flux_lang::ast::Node::List`. Depth is
      unbounded *and* finite: every leaf is a declared parameter, so no recursion and no blob. The
      refusals are enumerated in the module doc (`op.rs`, *How a request body is shaped* §3) and each
      has a test in `crates/connector-flux/tests/body_arrays.rs`.
- [x] **A fixed-length envelope and a caller-supplied list are different problems. Say which you
      solved.** SendGrid's `personalizations` is usually a one-element array wrapping real fields;
      Cloudflare's `files[]` is a genuine list of caller values. Solving only the first is a legitimate
      scope and would unblock SendGrid; pretending it solved the second would not.
      → **The fixed-length envelope.** Indices come from the provider file, so an array's length is a
      property of the declaration. `body_arrays.rs::a_caller_supplied_list_of_objects_is_still_not_decomposable`
      is that scope written as a test.
- [x] **Failing-first test:** a provider declaring an array body field does not load or does not emit
      today.
      → `body_arrays.rs::an_indexed_wire_path_builds_a_json_array`. At `dd8d21a` it loaded and emitted
      `payload = { "content[0]": { … }, "personalizations[0]": { "to[0]": { … } } }` — object keys
      carrying brackets, a 400 the vendor answers.
- [x] Every existing operation's emitted module is **byte-identical** — 27 providers, so no object body
      may change shape.
      → `cargo run -p connector-cli -- build` → *"54 providers, 945 artifacts up to date; nothing
      written"*; `diff` → *"945 artifacts up to date (54 providers checked)"*. (27 was the catalogue
      when this story was written; re-measured 2026-08-02.)
- [ ] `sendgrid-mail-send` ships, or this story records why it still cannot. It is the concrete case
      that motivated the work and the natural proof.
      → **Recorded, not shipped, and the reason moved.** The emitter now builds the envelope
      (`crates/connector-flux/tests/sendgrid_connector.rs::the_excluded_envelope_shape_is_now_expressible`).
      What blocks the operation is the **host**: `connector_pack`'s body evaluator has arms for
      `Lit`, `Var`, `Fmt`, `Obj` and `Parse` and refuses everything else — *"its body computes a
      list, which this pack does not evaluate"* (`crates/connector-pack/src/request.rs:1297-1332`,
      `kind` at `:1550`). Shipping the operation before that lands would put a catalogued,
      uncallable operation in the tree, which is the C-110 shape exactly. See *What is left* below.
- [x] Generated Flux still parses, analyzes and is a fixed point of flux's formatter, and the build
      stays a fixed point.
      → `body_arrays.rs::an_envelope_body_parses_analyzes_and_is_canonical`, and the fixed-point
      property is now **constructive** rather than asserted — see *What changed* below.

## Notes

- **Check what `flux_lang::ast` can express before designing the TOML surface.** An array literal
  containing interpolated values may or may not be constructible the way `fmt` builds a string today;
  that constraint should shape the spelling rather than be discovered after it.
- Coordinate with [C-144](C-144-request-body-encoding.md): a `form` body **refuses** nesting outright,
  so arrays must stay refused there. Whatever lands must not accidentally make `form` accept one.
- **Do not confuse this with [C-56](C-56-optional-body-fields.md).** C-179 hit an adjacent wall and
  cited the right story: Front's optional `to`/`cc`/`bcc` are flat arrays this pipeline *can* build, but
  an optional body field cannot be omitted without sending an explicit `null`. That is C-56's problem,
  not this one, and conflating them will send an implementor down the wrong path.
- Optional fields inside an array element are the sharp edge — `when` guards work for a flat body, and
  an omitted field inside an array element cannot leave a hole. Say what happens.
- This runs solo: it changes body lowering, which every provider reads.

## Progress

**2026-08-02, `impl/C-185` from `dd8d21a`.** Emitter only; no provider file changed, no generated
artifact touched.

### What changed

1. **A `wire` segment takes a bracketed index.** `key[0]` is element zero of the array `key` holds.
   `BodyNode` gained `Elements(BTreeMap<usize, BodyNode>)` beside `Leaf` and `Branch`, a path is
   parsed into `Step::Key`/`Step::Index` before insertion, and an `Elements` lowers to
   `flux_lang::ast::Node::List`. An index may address a whole element (`properties.title[0]`) as
   well as a field inside one.
2. **Six refusals, each because the alternative is a request a vendor answers 200 or 400 to**:
   a hole in the indices (`SparseBodyArray`); a bare numeric segment (`NumericWirePathSegment` —
   `items.0.value` used to build an object keyed `"0"` and now sits one character from a spelling
   that means something else); any other bracket spelling, array-of-arrays included
   (`BadArrayIndex`); a body whose *root* is an array (existing `BadWirePath`, via the empty first
   segment); an indexed path under `form` encoding (existing `UnencodableFormField`, C-144's rule
   extended as the story asked); and a bracketed *name* with no `wire` (existing `NestedBodyField`,
   which now covers `[` for the reason it already covered `.`).
3. **The emitter now emits flux's CST spelling, not its AST printer's** (`op::canonical`). This was
   forced, not chosen: measured on flux-lang 0.47.1, `flux_lang::format` prints a list as `[ a, b ]`
   while `format_cst::format_module` — the formatter the C-11 canonicality gate compares against, and
   the one a human editing the file runs — prints `[a, b]`. A record is `{ a: b }` in both, which is
   why nothing hit this before. Without the round trip, *any* module carrying an array fails the
   canonicality gate. It is safe by construction: `format_module` has an equivalence guard
   (`format_cst.rs:51-61`) and returns `Some` only for text it has re-parsed and lowered to the
   identical module, so it can change spacing and never meaning; `None` becomes the new
   `Error::NotCanonical`. This is the same upstream disagreement `Error::UnspellableDuration` already
   records, resolved the other way because a list *has* a CST spelling and a duration does not.
   **It is inert on the whole catalogue** — `build` wrote nothing, `diff` reports no drift.

### What is left

- **`sendgrid-mail-send` is one pack change and one provider edit away.** `connector_pack`'s `eval`
  needs a `Node::List` arm (`crates/connector-pack/src/request.rs:1297`), which is the mirror of the
  `Node::Obj` arm above it; without it the C-233 gate refuses any operation whose body carries an
  array, so the first provider to use this capability turns
  `every_declared_operation_composes_a_request_from_its_declared_configuration` red. Both files were
  outside this story's write set. **Nothing in the catalogue uses an indexed path yet, so the gate is
  green today** — the trap opens for whoever writes the first one.
- `providers/sendgrid.toml`'s header (lines 25-60) states that `BodyNode` has exactly two variants
  and no numeric-index segment, and that an array-shaped `wire` path silently builds objects. Both
  halves are now false. `crates/connector-flux/tests/sendgrid_connector.rs` — which is the mechanical
  record that paragraph points at — was updated in this branch; the prose was not, because the
  provider file was outside the write set.
- The other four vendors this story names are unblocked to different degrees: Miro and Webflow bulk
  writes and Algolia's `requests[]` are fixed-length only if the operation declares a fixed number of
  items, and **Postmark's batch send is not unblocked at all** — a root array is a caller-supplied
  list, which is the half this story deliberately did not solve.
