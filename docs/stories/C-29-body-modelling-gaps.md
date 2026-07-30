---
id: C-29
title: Close the request-body modelling gaps in the IR
pillar: Spec
status: done
priority:
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec]
note: found by C-17 transcribing real providers · blocks correct write operations
---

# Close the request-body modelling gaps in the IR

## Goal
Let the IR describe the request bodies real vendors actually accept, so a generated write operation
sends a body the API will take rather than a flat approximation of one.

## Acceptance
- [x] **A body parameter records the JSON path it occupies.** `ParamSet.body` is a flat
      `Vec<Param>` with one `name`, but Zendesk's wire body is
      `{"ticket": {"comment": {"body": …}}}`. Without a path, every Zendesk write emits a flat body
      the API rejects.
      → `Param::wire` (`crates/connector-spec/src/ir.rs:113`), assembled by
      `op::body_tree` (`crates/connector-flux/src/op.rs`); golden
      `crates/connector-flux/tests/golden/zendesk-ticket-comment-add.flux`.
- [x] **A body field can be constant** — always emitted, never in the op signature. Zendesk always
      sends `ticket.safe_update = true`. Declaring it `required` with a JSON Schema `const` (the
      current workaround) leaks it into the signature as a parameter a model must pass.
      → already closed by C-9's `constant()`; it now works *at a wire path* too —
      `op_emitter.rs::a_body_field_travels_at_the_json_path_its_wire_names` asserts
      `safe_update` reaches `ticket.safe_update` and stays out of the signature.
- [x] **A free-form object body is expressible** — "the body *is* this schema", not "the body is
      these named fields". Two babelforce operations have `{"type": "object"}` bodies with no
      properties and currently ship with no body parameter at all.
      → `ParamSet::body_schema`; emitted as `$payload = parse($body, as: "json")`
      (`op_emitter.rs::a_free_form_body_is_canonicalized_before_it_is_sent`).
- [x] **A parameter can carry a wire name distinct from its caller-facing name** (Freshdesk's
      `req_id` → `requester_id`).
      → the same `Param::wire` field, applied to path/query/header as a plain alias
      (`op_emitter.rs::a_query_alias_travels_under_its_wire_name`,
      `…::a_header_alias_travels_under_its_wire_name`).
- [x] Every change is **additive** — no existing encoding changes, and C-2's determinism and
      round-trip tests keep passing unchanged.
      → both fields are `Option` with `skip_serializing_if`, so an IR that declares neither encodes
      to the same bytes. `tests/determinism.rs` and `tests/ir_roundtrip.rs` pass with **no assertion
      changed**; their struct literals gained `wire: None` / `body_schema: None` to compile, and
      nothing else.
- [x] `providers/zendesk.toml` can express its real bodies without recording the shape in a
      `description` string.
      → every body field of the three `PUT` operations now declares `wire`; no `description`
      mentions a wire path.

## Progress
- **The exact fields are now known**, named by C-9 after running all 25 real operations through the
  emitter:
  - **`Param::wire: Option<String>`** — `#[serde(default, skip_serializing_if = "Option::is_none")]`.
    A dot-separated JSON path for a body field (`ticket.comment.body`), and a plain alias for query
    and header parameters. **One field closes gaps 1 and 4 together.**
  - **`ParamSet::body_schema: Option<JsonSchema>`** — for "the body *is* this schema" (gap 3).
- **Gap 2 is already closed** without an IR change: C-9 reads a JSON Schema `const` on a body field
  as a constant — emitted into the payload, kept out of the op signature.
- **Gap 1 is silently wrong for Zendesk today, and that is the urgent part.** C-9's emitter refuses
  `babelforce-agent-status-update` loudly because `presence.name` *looks* like a path. It cannot
  refuse Zendesk: `providers/zendesk.toml` carries the caller-facing name in `name` and the wire path
  only in `description`, so `zendesk-ticket-update`, `zendesk-ticket-comment-add` and
  `zendesk-tag-add` would emit a **flat body Zendesk ignores** — a silent wrong result, not an error.
  Nothing in the IR distinguishes that from a genuinely flat body.

- **Implemented on `impl/C-29`.** Both fields landed as C-9 specified them; the emitter builds a
  nested `$payload` from `wire` paths, and all three shipped providers now compile. The build had
  never produced a single artifact before this — `connectors/{zendesk,freshdesk,babelforce}.flux`
  plus their manifests are committed here for the first time.
- **The silent Zendesk failure is closed and pinned.** `zendesk-ticket-update`,
  `zendesk-ticket-comment-add` and `zendesk-ticket-tag-add` emit
  `$payload = { ticket: { … } }`, including `ticket.additional_tags` for the tag write — sending
  `tags` would have *replaced* the ticket's tags. Pinned twice: a golden in `connector-flux`, and
  `connector-cli/tests/shipped_providers_build.rs::zendesk_writes_a_nested_body`, which reads the
  real `providers/` through the real pipeline.
- **Three refusals were added rather than resolutions**, because each alternative silently drops a
  field an author declared: a dotted body `name` with no `wire` (undecidable), a `wire` path with an
  empty segment, and two fields whose paths need one node to be both a value and an object.
  `body_schema` alongside named `body` fields is refused for the same reason.
- **A free-form body is `parse(…, as: "json")`, not `body: $body`.** A composite op's parameter is
  stored with `Value::from_json` (flux-lang `runtime.rs:313-331`), so a caller-supplied record
  arrives as a `Value::Struct`; `http.request` reads `body` with `Value::as_str`, and would have
  sent **no body at all**. `parse` canonicalizes a record and validates a JSON string, storing text
  either way.
- **`providers/freshdesk.toml` now carries the caller-facing names** `req_id`/`req_email`/`updated`
  with the wire names in `wire`, matching the inventory's own two-column table (§4.2 op 2). This
  changes `freshdesk-ticket-list`'s public parameter names; it is what this story's Acceptance
  names, but it is the one change here that is a judgment call rather than a correctness fix.
- **Two new whole-inventory tests** guard against regression:
  `connector-cli/tests/shipped_providers_build.rs` (all three providers compile through the real
  pipeline) and `connector-flux/tests/shipped_modules.rs` (all 25 real operations emit, parse, are
  formatter fixed points, and reload as composite ops).
- Still open and deliberately untouched: Freshdesk's missing credential (C-17),
  `zendesk-ticket-search`'s unencoded query values (C-28/C-30), Zendesk's preflight precondition,
  and `base_url`'s unbound `{subdomain}`.

## Notes
- **Found by C-17 while transcribing the three real providers** — these are not hypotheticals, they
  are the four shapes that blocked writing correct definitions. Each is currently recorded in a
  parameter's `description`, which codegen cannot compile.
- Sequencing: C-9 owns body *emission* and is running now. If C-9 refuses these shapes with a named
  error rather than emitting something plausible-but-wrong, that refusal is the right outcome and
  this story removes the need for it.
- Gap 1 is the load-bearing one. The other three are each expressible as one additive field.
- Two further gaps C-17 found are **not** in this story because they are not about bodies:
  a `base_url` template variable has no declared env binding, and a hand-authored connector cannot
  record provenance without a `[spec]` table. Both deserve their own story.
