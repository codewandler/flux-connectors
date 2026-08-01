---
id: C-164
title: Ship the Algolia connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "unblocked and shipped by C-229, which made the declaration writable: `ConfigField::also_binds` lets one collected value reach more than one request position. `providers/algolia.toml` declares the application id once — `binds = \"endpoint.app_id\"`, `also_binds = [\"header.X-Algolia-Application-Id\"]` — so one question, one host-side slot, one answer reaches the hostname and the header, and the emitted module carries `{app_id}` in both. Five curated operations, all with response schemas. C-164's two boundary measurements are kept as tripwires in crates/connector-flux/tests/algolia_connector.rs rather than deleted: two *fields* under one name are still refused as a shared slot, and a header pin still does not bind a `base_url` variable."
---

# Ship the Algolia connector

## Goal

Add Algolia to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A configured host plus a second credential.** Algolia sends `X-Algolia-API-Key` and `X-Algolia-Application-Id`, and the application id *also* forms the hostname. One declared value has to reach two places.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** Two headers: `X-Algolia-API-Key` (secret) and `X-Algolia-Application-Id` (not secret).

**Curated operation set (a starting point, not a mandate):** search an index, get an object, list indices, save an object, delete an object (destructive)

## Hazards specific to this one

The application id is **not** a secret, so `secret` must disagree with the API key's — the configuration contract requires `secret` to agree with `binds`, so get that pairing right. Depends on the same configured-host question as [C-163](C-163-provider-salesforce.md); coordinate rather than both discovering it.

## Acceptance

- [x] `providers/algolia.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. **Done, once C-229 made the
      declaration writable.** Five operations: `algolia-index-list`, `algolia-index-search`,
      `algolia-object-get`, `algolia-object-save`, `algolia-object-delete`. The application id is one
      `[[config]]` field reaching both destinations — `binds = "endpoint.app_id"`, `also_binds =
      ["header.X-Algolia-Application-Id"]` — so an operator answers one question and the emitted
      module carries `{app_id}` in the hostname *and* in the header literal.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. The delete is `destructive` and claims no repeat
      guarantee; the save is a `PUT` declared `conditional` with `repeatable_because` stated, because
      the write is asynchronous and a stored result must never stand in for running it.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      Two fields: the API key (`secret = true`, `credential.algolia.api_key`) and the application id
      (non-secret, two destinations).
- [x] A `verify` operation that is a read and runs unattended — `algolia-index-list`, a plain `GET`
      with no required parameters, which exercises exactly the application-id/key pair this
      connector's configuration is about.
- [x] `crates/connector-flux/tests/algolia_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. **Rewritten
      by C-229 around the shipped provider**: `the_application_id_is_one_question_reaching_two_positions`
      and `the_two_destinations_carry_one_placeholder_into_the_emitted_module` are the acceptance
      assertions. C-164's two boundary measurements were **updated deliberately rather than deleted** —
      `one_name_for_both_destinations_is_refused_as_a_shared_slot` and
      `a_header_pin_does_not_bind_the_hostname_template` both still refuse, each now with the
      contrasting `also_binds` declaration beside it.
- [x] **Failing-first test:** the contract test must fail before `providers/algolia.toml` exists.
      **Satisfied by C-229's own failing-first test**, which is the declaration this connector needed:
      `one_field_declares_two_destinations_and_one_value_reaches_both`
      (`crates/connector-spec/tests/config_fields.rs`) does not compile at the merge base, because
      `also_binds`, `ConfigField::bindings`, `::slot`, `::pins` and `Pin` do not exist there. See
      C-229's `BASE_PROOF`.
- [x] The scoped gate is green: `build --provider algolia`, `diff --provider algolia` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced.** **Four were red, not eight, and each
      is named in C-229's report with what was decided about it** — the whole-catalogue staleness
      checks went green again on `cargo run -p connector-cli -- build`, which C-229 ran and committed.
      The four: `every_shipped_configuration_variable_is_placed` (predicted in `Slot::Unplaced`'s own
      doc comment and decided there), `every_declared_operation_composes_a_request_from_its_declared_configuration`
      (its "headers never move" clause predates C-187 and is now "only where the provider file declares
      a pin"), `the_known_rfc_idempotent_divergence_from_flux_has_not_grown` (answered by declaring the
      save `conditional` rather than growing the divergence), and this file's own
      `no_provider_toml_was_shipped_for_this_probe`, deleted because the finding it recorded is
      overturned.

## Progress

- **2026-08-01 — shipped, by C-229.** The second attempt's `## What would unblock it` was written as a
  new story and that story landed: `ConfigField::also_binds` lets one collected value reach more than
  one request position, keeping one field, one `name`, one host-side slot and one question — which is
  exactly what the shared-slot rule protects and what this connector needs.

  The narrow shape the second attempt proposed is the one that shipped, and the design interaction it
  flagged — *"`Position`'s `name` is deliberately the placeholder **and** the wire spelling, so a
  multi-destination field needs a story about which placeholder the emitted module carries"* — was
  settled rather than discovered late: **the emitted module carries `binds`' own target everywhere**,
  and a further destination contributes only the spelling the vendor sees. `providers/algolia.toml`'s
  emitted operations bind `X_Algolia_Application_Id = "{app_id}"` and send it as
  `"X-Algolia-Application-Id"`, beside `base = "https://{app_id}.algolia.net"` — one variable, two
  positions, one value a host resolves.

  Two things this file settled that the second attempt did not anticipate:

  - **One service, on `{app_id}.algolia.net`, not two.** Algolia's `-dsn` host is a read-optimised
    replica, and splitting reads onto it would need a second `[[services]]` — and therefore a second
    application-id `[[config]]` field, because a field belongs to exactly one service. That is the
    same two-slot defect one layer up, on the same value, so the file ships one service on the
    primary host and says so.
  - **`algolia-object-save` is `conditional`, not `idempotent`.** Algolia's `PUT` is idempotent in
    effect, but the write is asynchronous and every call answers with a fresh `taskID`, so a stored
    result must never be served in place of running it. Declaring flat `idempotent` would also have
    grown the `PUT`/`DELETE` population that diverges from flux's I3 coherence rule, which is a filed
    conflict this file had no reason to join.

- **2026-07-31 (second attempt, after C-187 landed) — the block half lifted, and the story is still
  blocked on the half that remains.** Re-measured against the loader, not re-read off the note.

  **What C-187 removed.** The original blocker's first clause is gone.
  `Binding::Request { position: Position::Header, name }` exists
  (`crates/connector-spec/src/config.rs:237`), `parse_binding` accepts `header.<name>`, and
  `Binding::is_secret` returns **false** for it (`config.rs:426-438`) with a doc comment that names
  this case directly: a pinned request value "travels in a URL or a header the module itself
  composes, where a secret must never be". `Position`'s own documentation cites *this story's*
  header as one of its three motivating vendors. So the application id no longer has to be
  mislabelled as a credential to reach `X-Algolia-Application-Id`, and
  `a_config_field_reaches_a_header_without_routing_through_auth` plus
  `a_pinned_header_reaches_the_emitted_request_and_not_the_signature` prove it reaches the emitted
  wire, not just the declaration. **Finding 1 of the original three is overturned; finding 2 is now
  avoidable rather than merely refused.**

  **What still blocks, stated more sharply than the first attempt could.** The original note's last
  sentence — *"the hostname and the header cannot share one declared value"* — is still true, and it
  is now the whole of the blocker rather than a consequence of the first clause. `binds` names
  exactly one destination, and Algolia's application id needs two: the hostname
  (`{app_id}-dsn.algolia.net`) and the header, on every call. The three ways to make them share are
  now each **measured**, which is what this attempt adds:

  | shape | outcome | pinned by |
  |---|---|---|
  | two fields, different names — `endpoint.app_id` + `header.X-Algolia-Application-Id` | **loads** | `the_hostname_and_the_header_are_still_two_declared_fields_with_two_slots` |
  | two fields, one name — both spelled `X-Algolia-Application-Id` | **refused** | `one_name_for_both_destinations_is_refused_as_a_shared_slot` |
  | one field, header pin alone, hostname resolving from it | **refused** | `a_header_pin_does_not_bind_the_hostname_template` |

  The middle row is the finding. The declaration that would give Algolia exactly what it needs — one
  collected value substituted into both positions — is refused **by name**, by `validate_pin`'s
  shared-slot pass (`crates/connector-spec/src/provider.rs:795-820`): *"both resolve `{app_id}` in
  service `default`, so a host would key them to one value under one slot. Two questions that share
  an answer are one question."* The rule is correct on its own terms — two *fields* keyed to one
  slot means one field's answer is silently discarded — and it is precisely what makes this
  connector unshippable. The rule and the vendor want opposite things, and there is no way to write
  the one question that would satisfy both. The bottom row closes the last escape: only
  `Binding::Endpoint` binds a `base_url` variable (`provider.rs:831-855`), so a header pin cannot
  stand in for the hostname.

  **Why the top row is not "good enough".** It loads, so shipping was a real option, and it was
  weighed rather than dismissed. It fails on the configuration contract's own terms: a second
  `[[config]]` field for the same value has no honest `label`/`help` — the only truthful help text
  is *"type the same value again"* — against a rule that a connector "asks for everything it needs
  and nothing it cannot use", and a rule that a field must be renderable. It would also be a
  deliberate circumvention of the shared-slot refusal above, passing only because `app_id` and
  `X-Algolia-Application-Id` are different *strings*. And the failure mode is the bad one: an
  operator who typos one of the two gets a DNS error or an Algolia `403` that neither declaration
  explains.

  **Algolia is the only one of C-187's three motivating vendors whose scope sits in two positions.**
  Cloudflare's `zone_id` is a path segment; Vercel's `teamId` is a query parameter; each sits in
  exactly one place, which is why both ship and this does not. C-187 was not wrong to cite Algolia —
  it delivered the header the note asked for — it just could not deliver the sharing, and the
  sharing is what this connector needs.

  **Still no `providers/algolia.toml`, and no operations authored**, for the reason the first attempt
  gave and which has not changed: authoring paths, schemas and risk/idempotency for a connector that
  cannot express its own configuration honestly would need re-deriving once the config question is
  actually answered. Zero new red whole-catalogue tests, again, which is the correct count for a
  story that shipped no provider.

  **What would unblock it** — and this is a new story, not this one: a `[[config]]` field that can
  declare **one value reaching more than one position**. The narrow shape that fits the existing
  model is to let a single field name a set of destinations rather than one (`binds` becoming a list,
  or an `also_binds`), keeping one field, one `name`, one host-side slot, one question — which is
  what the shared-slot rule is protecting and what Algolia needs. Note the interaction to design
  against: `Position`'s `name` is deliberately "the placeholder *and* the wire spelling", so a
  multi-destination field needs a story about which placeholder the emitted module carries when the
  two destinations spell it differently.

- **2026-07-31 (first attempt) — blocked at the config surface. Nothing shipped, deliberately.** The probe
  question this story exists to answer — *can one declared value reach both the hostname and a
  header?* — is **no**, and it was measured against the loader rather than read off the design doc.

  Two of the three prerequisites were already answered before this story ran, so this Progress note
  does not re-derive them: two credentials on one request is expressible (`AuthRequirement::all`, C-160
  / Datadog) and a configured host is expressible (`Binding::Endpoint`, C-163 / Salesforce). What
  remained was whether Algolia's application id — required in *both* the hostname
  (`{app_id}-dsn.algolia.net`) and the `X-Algolia-Application-Id` header, and **not secret** — could be
  declared once and reach both.

  1. **`ConfigField::binds` parses to exactly one of five destinations, and a request header is not
     one of them.** `crates/connector-spec/src/config.rs:178-202`:

     ```rust
     pub enum Binding<'a> {
         Endpoint { variable: &'a str },
         Credential { name: &'a str },
         Username { name: &'a str },
         OAuthClientId,
         OAuthClientSecret,
     }
     ```

     `parse_binding` (`config.rs:239-267`) accepts only `endpoint.`, `credential.`, `username.`,
     `oauth.client_id` and `oauth.client_secret`, and refuses everything else —
     `crates/connector-flux/tests/algolia_connector.rs::config_binding_has_no_header_destination`
     measures it directly against a `header.`-shaped string that was never given a spelling.
  2. **The one route that *can* place a value in an arbitrary request header — a declared `[[auth]]`
     credential — forces `secret = true` on whatever `[[config]]` field binds it, unconditionally.**
     `Binding::is_secret` (`config.rs:223-231`) returns `true` for `Credential` with no exception, and
     the loader enforces the agreement rather than trusting it
     (`crates/connector-spec/src/provider.rs:609-629`). A `[[config]]` field binding
     `credential.algolia.application_id` while declaring `secret = false` — the true fact, since
     Algolia documents the application id as safe to embed in client-side code alongside a
     search-only key — is refused for exactly that contradiction. `algolia_connector.rs`'s
     `routing_the_application_id_through_a_credential_forces_a_false_secret_claim` proves the refusal
     fires rather than assuming it.
  3. **The endpoint binding reaches the hostname and nothing else.** Binding `endpoint.app_id` loads
     cleanly and resolves `{app_id}` in `base_url` — this is exactly the shape C-163 shipped. But
     `ParamSet::header` (`crates/connector-spec/src/ir.rs:259-266`) is a **caller-supplied** parameter,
     filled in by a model on every call, with no link back to `[[config]]` at all. Declaring the same
     header there does not pin it to the config value; it only gives the operator (or a model acting
     for one) a second, unconnected place to type the same string.
     `the_endpoint_binding_reaches_only_the_host_and_a_header_parameter_is_a_separate_per_call_value`
     measures this: the binding resolves correctly, and no operation in the fixture has any way to
     reach it from a header.

  So the two positions cannot share one declared value today, and the two ways to *not* share it are
  both bad: asking the operator to paste the application id twice (once as `endpoint.app_id`, once as
  a mislabelled `credential.algolia.application_id`) risks a silent mismatch that produces a confusing
  vendor error neither declaration explains: or omitting the header entirely and shipping a connector
  that fails closed on every real call, which the story's own framing ranks below a recorded refusal
  ("that attempt was worth more than a connector that answered 400"). Recording the refusal was the
  chosen path.

- **Filed as a finding for [C-187](C-187-config-cannot-pin-a-request-component.md).** That story
  already tracks two motivating cases where `ConfigField::binds` cannot reach a request component —
  Cloudflare's `zone_id` (a path segment, C-169) and Vercel's `teamId` (a query parameter, C-170) — and
  its own Notes already flag the header case as worth checking: *"Worth checking while here: whether a
  **header** can be operator-pinned... nothing pins a header the operator knows."* This story is the
  answer to that open question, met by a real connector rather than a hypothetical. C-187 is a shared
  ledger this story's fence does not permit editing directly, so the finding is recorded here for the
  coordinator to fold in at integration: a non-secret, operator-known value has no route into a
  request header today, the same gap C-187 already names for a path segment and a query parameter.
- **No `providers/algolia.toml` shipped, and no crate other than the new test file touched.** Shipping
  a connector whose only way to send the required header would be to either duplicate the value under
  a false `secret = true` claim, or omit the header and fail closed on every call, is the exact failure
  mode AGENTS.md's non-negotiable rules exist to avoid ("a loud compile-time refusal is better than
  plausible but incorrect Flux"). The curated operation set the story suggested (search an index, get
  an object, list indices, save an object, delete an object) was not authored against endpoints for the
  same reason C-107's and C-161's Progress notes give: authoring paths, schemas and risk/idempotency
  for operations that cannot authenticate honestly would be effort spent on a shape that cannot ship,
  and every one of them would need re-deriving once the config question is actually answered.
- **The eight-red / three-red whole-catalogue pattern does not apply here.** No provider, service or
  operation was added to `providers/`, so `cargo test --workspace --no-fail-fast` shows the
  whole-catalogue staleness checks green, not red — see `GATE` in the report. That is itself part of
  the finding: this story closes with **zero** new red tests, which is the signal that nothing was
  shipped, rather than the eight AGENTS.md tabulates for a story that did.
- **Board not regenerated** — `docs/stories/README.md` is coordinator-owned. `status` moved `ready` ->
  `blocked` here, so the board needs a `/track:board` run at integration.

## Notes

- **Charter fit.** Algolia is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
  non-goals exclude technology adapters and the inference path. If authoring reveals it is really a
  technology rather than a service, stop and say so rather than shipping it.
- **Provenance is hand-authored and drift is undetectable by machine**, the caveat zendesk, freshdesk,
  github and notion already carry. Record it in the TOML's header comment.
- **Do not invent an endpoint.** A confident set of four operations beats a guessed set of ten — the
  repository's rule is that a loud refusal beats plausible-but-incorrect output, and a hallucinated
  path ships as a `404` a contract test cannot catch. Where you are unsure of a path, shape or required
  field, leave the operation out and name it in `## Progress` as unverified.
- **No credential value, and no realistic-looking `example` on a secret field.** A placeholder shaped
  like a real token has already tripped GitHub's push protection and blocked a release.
- Whole-catalogue artifacts are coordinator-owned: `crates/catalog/src/generated.rs`,
  `web/public/catalog.json`, `web/public/v1/**`, `assets/readme-snippet-*.svg`. The per-provider
  `crates/catalog/src/generated/algolia.rs` is **not** in that set and is yours to commit.

### Coordinator note at integration

Blocked on [C-187](C-187-config-cannot-pin-a-request-component.md), where this finding is now folded in
as its fourth and sharpest instance. The implementor was right not to edit C-187 itself — it is a shared
ledger outside this story's fence.

**Both of this story's original probes had already been answered elsewhere, and neither was the blocker.**
Two credentials on one request work (C-160 Datadog, via the AND-set in `AuthRequirement`); a configured
host works (C-163 Salesforce, via `Binding::Endpoint`). What stops Algolia is narrower and was not
predicted by the story: **one non-secret value cannot reach both a hostname and a header.**
