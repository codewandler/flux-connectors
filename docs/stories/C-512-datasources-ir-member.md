---
id: C-512
title: "The [[datasources]] member — namespace, derived schema, per-verb bindings and validation"
pillar: Spec
status: backlog
design: docs/designs/vendor-datasource-declarations.md
epic: vendor-datasources
areas: [connector-spec]
note: "the sixth member kind. A datasource member joins member_names_of, derives its entity schema from the IR, binds list/get to named operations with explicit param/filter/cursor/field mappings, and is refused at load when a binding dangles. Joins the HashDomain as compiled meaning"
---

# The `[[datasources]]` member — namespace, derived schema, per-verb bindings and validation

## Goal

Model the vendor datasource member in the connector IR so a connector can declare which entities a
vendor exposes and how each read verb executes as one of its own declared operations — with nothing
independent to retrieve through and nothing hand-written to drift.

## Acceptance

- [ ] `[[datasources]]` is a sixth member kind: it joins the per-service member namespace
      (`Connector::member_names_of`, `crates/connector-spec/src/ir.rs:1956` as of 2026-08-05), so a
      cross-kind collision is the same loud load error the other five produce, and a member renders
      into the `…#name` address form.
- [ ] An entity's schema is **derived from the IR** — the backing operations' declared response
      schemas — never hand-written. A test asserts no authored schema field exists to disagree with
      the derivation.
- [ ] Per-verb operation bindings: `list` names a declared operation of the same connector plus
      explicit parameter, filter, cursor and field mappings; `get` names a declared operation plus
      the record-id → id-parameter mapping. A verb the vendor cannot serve is omitted, not stubbed.
- [ ] Cursor/paging vocabulary lives on the binding and pins the one-shot spelling this
      repository already ships: `Pagination::Cursor`'s three fields — `cursor_param`,
      `next_cursor_pointer`, `max_pages` (`crates/connector-spec/src/ir.rs:524` as of
      2026-08-05). **This story fixes the one-shot cursor spelling, and
      [C-497](C-497-declare-runtime-operation-bindings.md) must not mint a second one**; C-512
      waits on C-497 only for the stream/tail/lease terms, which datasource v1 does not use.
      "Cursor" here is the paging of one `list` read — not the poll-channel *cursor operation*
      (`ChannelBinding::cursor`, `crates/connector-spec/src/inbound.rs:471` as of 2026-08-05),
      which names an operation a poll transport calls on a schedule.
- [ ] **Credential reach is the backing operation's declared auth only, never a value.** A member
      cannot name a credential, declare an auth block, or widen what its operations already reach.
- [ ] **The loader refuses a dangling projection**: a binding to an operation the connector does
      not declare, a mapping to a parameter the operation does not take, a cursor pointer into a
      response the operation does not declare, and a `get` binding with no id mapping each fail at
      load with the member and the missing name in the message — so `flux-connectors build` fails
      loudly rather than shipping a projection nothing can execute. **Failing-first tests**, one
      per refusal.
- [ ] **A read verb never binds a write** — the fifth load-time refusal: a `list` or `get`
      binding that names a `direction = "write"` operation fails at load, naming the member and
      the operation. The check reads the resolved `Operation::direction`
      (`crates/connector-spec/src/ir.rs:1151` as of 2026-08-05, C-516) — never `patch.directions`,
      which is ingest input the resolver has already folded into that field. **Failing-first
      test** of its own.
- [ ] **A verb binding requires a declared response shape**: a `list` or `get` binding to an
      operation that declares no `response_schema` is refused at load — the entity schema is
      derived from exactly that declaration, so its absence leaves nothing to derive from.
      **Failing-first test** of its own.
- [ ] `datasources` joins the `HashDomain` as compiled meaning — both the struct
      (`crates/connector-spec/src/ir.rs:2207` as of 2026-08-05) and the exhaustive destructuring
      in `HashDomain::of` (`ir.rs:2257`), where the new field is a compile error until classified;
      this story classifies it in.
- [ ] The same implementation diff corrects the three places that still count the member namespace
      at three: the `member_names_of` doc comment (`crates/connector-spec/src/ir.rs:1942`–`1946`
      as of 2026-08-05), the `validate_member_namespace` doc comment
      (`crates/connector-spec/src/provider.rs:4460`) and the cross-kind collision error string
      (`provider.rs:4508`–`4512`) must all say **six** member kinds once `[[datasources]]` lands.
- [ ] The provider-TOML JSON Schema (`crates/connector-spec/schema/provider-toml.schema.json`)
      gains `$defs` entries for the datasource member and its binding/mapping objects, and each
      new object is registered in `accepted_keys()` (`crates/connector-spec/src/provider.rs:6013`
      as of 2026-08-05), so `tests/provider_schema.rs` holds the schema and the `Deserialize`
      impls to the same shape.
- [ ] The gate is green; the build stays a fixed point.

## Progress

- (not started)

## Notes

- Design: [vendor-datasource-declarations.md](../designs/vendor-datasource-declarations.md);
  authority is Decision 0006 rule 6. Emission is [C-513](C-513-publish-the-datasource-surface.md) —
  this story may land IR-and-loader-only, but per connector-surfaces.md the surface may not
  *release* IR-only, so the two ship in the same release.
- **No independent retrieval contract.** Every read executes as an admitted operation; this member
  carries mappings, never requests.
- Streaming is deliberately absent: datasource v1 is one-shot list/get with opaque cursors
  (Decision 0006 rule 10); tail/stream capability waits for the Milestone 3 vocabulary.
