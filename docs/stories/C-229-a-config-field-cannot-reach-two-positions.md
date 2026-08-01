---
id: C-229
title: "A configuration field cannot declare one value reaching two positions, and it is the only thing still blocking Algolia"
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/connector-configuration.md
epic:
areas: [connector-spec]
note: "measured by the C-164 implementor 2026-07-31, which refused to ship rather than declare the same question twice. Verified by the coordinator at provider.rs:806-811 — the declaration Algolia needs is refused by name: 'Two questions that share an answer are one question'"
---

# A configuration field cannot declare one value reaching two positions

## Goal

Let one collected value be substituted into more than one request position, so a vendor that puts its
tenant scope in two places can be expressed without asking the operator the same question twice.

## What was measured

[C-164](C-164-provider-algolia.md) was blocked on two clauses. **[C-187](C-187-config-cannot-pin-a-request-component.md) removed the first** — `Position::Header` exists at
`crates/connector-spec/src/config.rs:237`, `parse_binding` accepts `header.<name>`, and
`Binding::is_secret` returns `false` for `Self::Request { .. }`, so Algolia's application id no
longer needs a false `secret = true`. `Position`'s own documentation names this story's header as one
of its three motivating vendors.

**The second clause is untouched and is now the entire block.** Algolia's app id must reach the
`{app_id}-dsn.algolia.net` hostname *and* the `X-Algolia-Application-Id` header on every call. All
three ways to make one value reach both were measured:

| shape | outcome |
|---|---|
| two fields, different names (`endpoint.app_id` + `header.X-Algolia-Application-Id`) | **loads** — and is the problem: two host-side slots, one answer |
| two fields, one name | **refused** — shared-slot pass, `crates/connector-spec/src/provider.rs:795-820` |
| one field, header pin alone, hostname resolving from it | **refused** — only `Binding::Endpoint` binds a `base_url` variable, `provider.rs:831-855` |

The middle row is the finding, and the refusal is by name:

> configuration fields `X` and `Y` both resolve `{app_id}` … so a host would key them to one value
> under one slot. **Two questions that share an answer are one question** — bind one of them to a
> different name

## The rule is right; it is the expression that is missing

That refusal exists for a good reason (C-197): two fields keyed to one slot silently discard one
answer. It is not a bug to remove. But it is exactly what makes this connector unshippable, and the
rule and the vendor want opposite things.

The top row was weighed rather than dismissed. It fails on the configuration contract's own terms:
the second field has no honest `label` or `help` — the only truthful help text is *"type the same
value again"* — against the standing requirement that a connector asks for everything it needs and
nothing it cannot use.

**Why Algolia specifically:** of C-187's three motivating vendors it is the only one whose tenant
scope sits in *two* positions. Cloudflare's `zone_id` and Vercel's `teamId` sit in one each, which is
why those ship and this does not.

## Acceptance

- [x] **Failing-first test:** one configuration field declares two destinations and one collected
      value reaches both. It cannot be declared today. Name it. →
      `one_field_declares_two_destinations_and_one_value_reaches_both`
      (`crates/connector-spec/tests/config_fields.rs`). At the merge base it does not compile:
      `also_binds` is an unknown key, and `ConfigField::bindings`, `::slot`, `::pins` and `Pin` do not
      exist.
- [x] The shape keeps **one field, one `name`, one host-side slot, one question** — that is what the
      shared-slot rule protects, and a fix that reintroduces two slots has solved nothing. A `binds`
      list, or an `also_binds`, are the candidates; record why the chosen one wins. → `also_binds`
      won, and the argument is recorded in `config.rs`'s module docs and in the design doc's
      `also_binds` section: **a list of peers has no head, and this declaration needs one.** With a
      head the placeholder rule is unconditional (the emitted module carries `binds`' target), the
      slot is provably one, and `binding()`/`level()`/the stored `(kind, name)` address stay exactly
      what they were for every field that existed before.
- [x] **Settle which placeholder the emitted module carries when the two destinations spell the value
      differently.** → The **head's**: `ConfigField::slot()` is `binds`' own target, and a further
      destination contributes only the spelling the vendor sees. `connector_spec::Pin` carries the
      two apart (`name` is the wire spelling, `variable` is the placeholder) so an emitter cannot
      pick the wrong one by accident. Asserted against the artifact in
      `the_two_destinations_carry_one_placeholder_into_the_emitted_module`.
- [x] The shared-slot refusal still fires for genuinely distinct fields that collide. Widening it into
      a hole is the failure mode; C-164's two boundary tests are the tripwires and must be updated
      deliberately, not deleted. → Both kept, both still refusing, each now with the contrasting
      `also_binds` declaration asserted beside it. The rule now compares **slots**
      (`validate_slot_is_not_shared`), which is what it always meant, and gained a second clause for
      the hole a further destination opens: two fields with two slots writing one wire position
      (`two_fields_writing_one_header_are_refused`).
- [x] `providers/algolia.toml` ships, C-164 closes, and its `status` moves off `blocked`. → Shipped,
      five curated operations, `945 artifacts up to date (54 providers checked)`. C-164 is `done` with
      every acceptance item ticked and a Progress note recording the two things it settled that its
      own second attempt did not anticipate.
- [x] Interaction with [C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md) is stated: one
      value reaching a hostname *and* a header must satisfy **both** position predicates, and the
      host predicate is the strict one. → `config::validate_host_value` is the host predicate at the
      loader, `Binding::validate_value` dispatches over every destination, and the loader applies it
      to the `example` and to every choice **once per destination**. At runtime `connector-pack`'s
      `Slot::Unplaced` is the intersection of every rule at once, host included, and substitutes the
      value unchanged — so nothing is encoded differently per destination. Both directions are pinned:
      `a_host_value_is_refused_for_what_no_request_position_would_catch` shows
      `acme.example@evil.example` passing all three request rules and failing the host rule.

## Notes

- This is the third distinct gap the 2026-07-31 wave found in the configuration surface, and they are
  genuinely different: [C-225](C-225-a-config-field-cannot-declare-a-closed-set-of-values.md) is
  about the set of legal *values*, this is about the set of *destinations*, and C-214 is about
  *validating* the value where it is substituted. Read all three before designing any of them — one
  change could serve two, and three separate spellings would be the defect they each describe.
- C-164 is a **second documented refusal** and that is a successful outcome, not a failure. It now
  refuses with the space closed rather than surveyed, which is what makes this story writable.

## Progress

- **2026-08-01 — landed. Algolia ships.** `ConfigField::also_binds` is the declaration: `binds` names
  one destination and stays the head, `also_binds` names the further request positions, and the field
  keeps one `name`, one `label`, one `help`, one row in a form and one host-side slot.

  **The rule the story was written to protect survives, and was sharpened rather than widened.** The
  C-197 shared-slot pass moved out of `validate_pin` into `validate_slot_is_not_shared`, run once per
  field rather than once per pin, and it now compares `ConfigField::slot()` — which is what "one
  host-side slot" always meant. For every field that existed before this landed the comparison is
  byte-for-byte the one it replaced, because a single-destination field's slot *is* its binding
  target. C-164's `one_name_for_both_destinations_is_refused_as_a_shared_slot` still refuses, with
  the same message, and now asserts the contrasting `also_binds` declaration loads in the same test.

  A further destination opened one hole the old rule could not see — two fields, two *different*
  slots, one wire position — so the function gained a second clause for it. That is a request
  carrying one of two values depending on an order nothing declares; `connector-flux` already refused
  the emitted shape (`Error::HeaderConflict`), and this is the declaration-level half that names the
  two fields rather than an operation.

  **Four tests went red on the shipped provider, and each was decided rather than adjusted:**

  1. `connector_pack::request::tests::every_shipped_configuration_variable_is_placed` — predicted, by
     name, in `Slot::Unplaced`'s own doc comment, which asked whoever landed this story to decide it
     on purpose. Decision: `Unplaced` is the right arm — it is the intersection of every position's
     rule, host included, and does not encode — and the test now carries a **named list** of the
     variables that reach every position, so an *accidental* arrival there is still red, and a listed
     variable that stops being multiply placed is red too.
  2. `connector_pack::tests::request::every_declared_operation_composes_a_request_from_its_declared_configuration`
     — its "headers never move when configuration does" clause predates C-187 and was already
     asserting the opposite of a declared feature; no shipped provider had exercised it. It is now
     "no header moves except one the provider file declares as a pin", derived from the provider file
     rather than listed, *and* the pinned header is asserted to move rather than merely permitted to.
  3. `connector_pack::tests::metadata_coherence::the_known_rfc_idempotent_divergence_from_flux_has_not_grown`
     — `algolia-object-save` is a `PUT`, and declaring it flat `idempotent` would have grown a filed
     conflict with flux's I3 from nine operations to ten. Answered in the provider file instead:
     `conditional` with `repeatable_because` stated, which is the truer claim anyway because the write
     is asynchronous and every call answers with a fresh `taskID`. The pinned population is untouched.
  4. `algolia_connector::no_provider_toml_was_shipped_for_this_probe` — deleted, because the finding
     it recorded is overturned and the file it named now exists.

  **Not done here, and named rather than done quietly:** `Format` still has no variant for "ten
  uppercase alphanumerics", so `providers/algolia.toml`'s application id is `format = "text"` with the
  shape in `help` — the same call `providers/cloudflare.toml` records for its 32-hex `zone_id`, and
  the missing `pattern` escape hatch `config.rs` has always declined to add without a vendor behind
  it. Algolia is now the second vendor behind it.
