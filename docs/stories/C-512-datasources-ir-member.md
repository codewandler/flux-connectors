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
      (`Connector::member_names_of`, `crates/connector-spec/src/ir.rs:1930` as of 2026-08-04), so a
      cross-kind collision is the same loud load error the other five produce, and a member renders
      into the `…#name` address form.
- [ ] An entity's schema is **derived from the IR** — the backing operations' declared response
      schemas — never hand-written. A test asserts no authored schema field exists to disagree with
      the derivation.
- [ ] Per-verb operation bindings: `list` names a declared operation of the same connector plus
      explicit parameter, filter, cursor and field mappings; `get` names a declared operation plus
      the record-id → id-parameter mapping. A verb the vendor cannot serve is omitted, not stubbed.
- [ ] Cursor/paging vocabulary lives on the binding and reuses
      [C-497](C-497-declare-runtime-operation-bindings.md)'s cursor/stream terms — one spelling,
      cross-referenced.
- [ ] **Credential reach is the backing operation's declared auth only, never a value.** A member
      cannot name a credential, declare an auth block, or widen what its operations already reach.
- [ ] **The loader refuses a dangling projection**: a binding to an operation the connector does
      not declare, a mapping to a parameter the operation does not take, a cursor pointer into a
      response the operation does not declare, and a `get` binding with no id mapping each fail at
      load with the member and the missing name in the message — so `flux-connectors build` fails
      loudly rather than shipping a projection nothing can execute. **Failing-first tests**, one
      per refusal.
- [ ] `datasources` joins the `HashDomain` destructuring (`crates/connector-spec/src/ir.rs:2181`)
      as compiled meaning — the field is a compile error there until classified, and this story
      classifies it in.
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
