---
id: C-187
title: "`[[config]]` can pin a base URL and nothing else, so an operator cannot scope a connector to one tenant"
pillar: Spec
status: in-progress
priority: 3
areas: [connector-spec, connector-flux]
note: "found twice in one wave: C-169 wanted an operator-pinned zone_id (a path segment) and C-170 wanted an operator-pinned teamId (a query parameter). ConfigField::binds reaches base_url and nothing else, so both became per-call arguments a model may set freely"
---

# `[[config]]` can pin a base URL and nothing else, so an operator cannot scope a connector to one tenant

## Goal

Let an operator pin a tenant-scoping value at install time, so it stops being an argument a model
chooses on every call.

## What was measured

Two connectors in one wave wanted the same thing and neither could have it:

| story | value | where it sits | outcome |
|---|---|---|---|
| [C-169](C-169-provider-cloudflare.md) | `zone_id` | a **path** segment on every operation | required per-call argument |
| [C-170](C-170-provider-vercel.md) | `teamId` | a **query** parameter | optional per-call argument |

`ConfigField::binds` reaches `base_url` and has no form that names a path segment or a query
parameter. Both implementors checked and recorded it rather than inventing a spelling.

## Why it matters more than convenience

**The two failure modes are different and both are real.**

For Cloudflare, one installed connector can address **every zone the token can reach**. That is
defensible — it is what the token permits — but it is not what an operator installing a connector for
one zone would expect, and there is no way to express the narrower intent.

For Vercel it is sharper, because `teamId` is *optional* and its absence is not neutral: omit it and the
call lands on the **personal account** instead of the team. So the connector ships a parameter whose
omission silently redirects a write, and the only mitigation available was to say so in the
`description` — which is text a model reads and may not act on.

## The header case, measured — and it blocks a connector outright

This story's Notes flagged *"whether a **header** can be operator-pinned"* as worth checking.
[C-164](C-164-provider-algolia.md) checked, and the answer blocked the Algolia connector entirely.
Algolia's application id must appear in the hostname (`{app_id}-dsn.algolia.net`) **and** as the
`X-Algolia-Application-Id` header. Three routes exist and all three fail, each measured against the
loader rather than reasoned about:

1. **`ConfigField::binds` cannot reach a header.** It parses to exactly five destinations —
   `Binding::{Endpoint, Credential, Username, OAuthClientId, OAuthClientSecret}`
   (`crates/connector-spec/src/config.rs:178-202`, `parse_binding` at `:239-267`). No header among them.
2. **The one route that does reach a header forces a lie.** An `[[auth]]`-declared credential reaches a
   header, but `Binding::is_secret` (`config.rs:223-231`) makes `secret = true` unconditional for any
   config field binding one, enforced at `crates/connector-spec/src/provider.rs:609-629`. An application
   id is **not** a secret, so this route buys the header at the cost of a false declaration — and
   `AGENTS.md` requires `secret` to agree with `binds` precisely so that field means something.
3. **`ParamSet::header` pins nothing.** It has no connection to `[[config]]` (`ir.rs:259-266`), so it
   only gives the operator a second, disconnected place to retype the same string — and a mismatch
   between the two produces a vendor error that neither declaration would explain.

**This is the instance that makes the story load-bearing rather than ergonomic.** Cloudflare and Vercel
shipped with a worse surface; Algolia cannot ship at all.

- [x] A non-secret, operator-supplied value can reach a request header. Note the shape of the problem:
      the fix is not "let a credential be non-secret" — that would weaken the `secret`/`binds` agreement
      that makes the credential path trustworthy — but a binding that reaches a header **without**
      routing through `[[auth]]`.

## A third instance, and it is about the config surface's own shape

[C-177](C-177-provider-contentful.md) hit a different limit in the same surface, measured against the
loader rather than guessed: **`validate_config` checks `ConfigField` name uniqueness across the whole
connector, not per service** — unlike operations, events and channels, which are per-service namespaces
(`AGENTS.md`'s member contract, *"one namespace per service"*).

So Contentful's two services could not each declare a `space_id`/`environment_id` pair. It ships four
fields — `delivery_space_id`, `delivery_environment_id`, `management_space_id`,
`management_environment_id` — where two would express the operator's actual intent, and an operator now
has to type the same space id twice with nothing checking they match.

That is worth folding into whatever this story decides, because it is the same question from the other
end: **the config surface is connector-scoped while almost everything else it interacts with is
service-scoped.**

- [x] Say whether `ConfigField` should be a per-service namespace like operations and channels are, and
      if so what happens to a field that legitimately spans services.

## Acceptance

- [x] A `[[config]]` field can bind a value that reaches a path segment and/or a query parameter, not
      only `base_url`. Decide which of the two you support and **record why** — they are not equally
      safe, and a query binding that silently changes a write's target account is the harder case.
- [x] **An operator-pinned value must not remain a caller argument.** If a value is pinned at install
      time, the emitted operation should not also accept it — otherwise the pin is advisory and a model
      can override the operator, which is the opposite of the point. Assert this.
- [x] The interaction with `Level` is stated: the configuration contract says *operator* level is
      derived, never authored, so pinning a tenant value has to land on the right level by derivation
      rather than by declaration.
- [x] **Failing-first test:** a provider binding a config field to a path segment does not load today.
- [x] `providers/cloudflare.toml` and `providers/vercel.toml` are revisited, or this story records why
      they stay as they are. They are the two motivating cases.
- [x] Every other provider's emitted module is byte-identical.

## Notes

- Read both connectors' `## Progress` notes first — each records the gap at the point it was hit.
- Do **not** solve this by letting a provider write a literal into a path template. A pinned value is
  operator data supplied at install time, not connector data known at compile time; conflating them
  would put a tenant id in the repository, which is the same category error as a credential value.
- Worth checking while here: whether a **header** can be operator-pinned. `const_headers` (C-55) pins a
  header the *connector* knows; nothing pins a header the *operator* knows, and an
  `X-Account-Id`-shaped vendor would want exactly that.

## Progress

**`binds` grew a closed vocabulary of three request positions, not something more general.**
`Binding::Request { position, name }` with `Position::{Path, Query, Header}` — spelled
`path.<variable>`, `query.<name>`, `header.<name>`. The reason for closing it is the reason
`BodyEncoding` is closed and `Format` has no `pattern`: an unknown spelling must be a **load error**
rather than a key the loader accepts and ignores, and a set with a variant behind no vendor stops
meaning anything. Three positions is exactly what the three measured vendors need — Cloudflare's
path segment, Vercel's query parameter, Algolia's header. A *body* position was considered and left
out: a body field the connector fixes is already expressible (a JSON Schema `const`, which
`connector-flux` sends without declaring), and no vendor met so far scopes a tenant through a
request body.

**Both of the two the acceptance asks about are supported, and the query one is safe only because
the pin is mandatory.** A path pin cannot fail open — the placeholder is either substituted or the
request refuses. A *query* pin can, and that is the whole Vercel hazard: `teamId` absent means
"personal account", so an optional pin would reintroduce the silent redirect with extra steps. So
the loader refuses `required = false` on any pinned field (`an_optional_pin_is_refused`), which
makes "the parameter was simply not sent" a state that cannot occur. The cost is stated rather than
absorbed: a personal-account Vercel installation is now out of scope for that connector, recorded in
its header comment.

**The pin is not advisory, and that is enforced twice.** The loader refuses a service whose
operations declare a parameter a pin already claims
(`a_value_that_is_both_pinned_and_declared_as_a_parameter_is_refused`,
`a_pinned_query_parameter_that_is_also_an_argument_is_refused`), and `connector-flux` refuses the
same shape independently over an IR another front-end could produce
(`Error::PinnedValueConflict`, `a_slot_claimed_by_both_a_pin_and_a_parameter_is_refused_at_emission`).
Neither precedence was safe to pick silently: honour the parameter and the pin is decoration, honour
the pin and a caller's value vanishes — and both produce a request the vendor answers `200` to,
addressed to a tenant nobody chose. A pinned header is refused through the existing
`Error::HeaderConflict`, which already compares every source that can claim a header name.

**`Level`: a pin is *connection* level, derived, and the story's word "operator" is about
timing rather than level.** "Operator" in this model means *once per vendor, by whoever runs the
product* — the app registration every tenant shares. A zone or a team is the opposite: one per
tenant, chosen by the person connecting their own account, and two tenants of one deployment pin two
different ones. Calling it operator level would put one customer's zone in front of every other
customer — the same conflation, in the same direction, as asking an end user for a client secret. So
nothing was authored: `Binding::level` returns `Connection` for `Request`, and the providers declare
no level at all.

**A pinned value is configuration, not a credential, and it is said in the doc-comment.**
`Binding::is_secret` is `false` for every `Request` binding, so the existing `secret`/`binds`
agreement refuses a field claiming otherwise (`a_pin_that_claims_to_be_secret_is_refused`), nothing
registers it with a redactor, and a header pin on `Authorization`/`Proxy-Authorization`/`Cookie` — or
on the header an `[[auth]]` credential is injected into — is refused outright
(`a_header_pin_on_an_auth_owned_header_is_refused`). That is the line C-164 asked for: a header
reached **without** routing through `[[auth]]`, rather than a credential allowed to be non-secret.

**The escape guard.** `Position::validate_value` refuses any value that would reshape the request it
lands in: for a path, `/`, `\`, `?`, `#`, `%`, whitespace and the segments `.` and `..`; for a query,
`&`, `=`, `?`, `#`, `+`, `%` and whitespace (nothing percent-encodes on the way out — C-30); for a
header, any non-ASCII or control byte, CR and LF above all, and leading/trailing whitespace. A brace
is refused everywhere, because substitution fills placeholders in emitted literals and a value
spelling one would be filled in twice. The loader applies it to `example` — the string a user copies
— so `example = "../admin"` does not load
(`a_path_pin_whose_example_escapes_its_segment_is_refused`), and the unit test
`a_pinned_value_cannot_reshape_the_request_it_lands_in` covers the predicate directly. **What it is
not yet:** a host-side gate. This repository never sees a real configuration value, so the guarantee
is only as good as the caller — see the last paragraph.

**How a pin reaches the wire, and why it is not a literal in this repository.** The emitter binds
`zone_id = "{zone_id}"` — a string literal carrying its own placeholder — immediately after `base`,
and the URL/header record reads that symbol. That is not a stylistic echo of `base =
"https://{subdomain}.zendesk.com"`; it is the same mechanism. `connector-pack` derives a connector's
configuration variables from the braces surviving in its emitted *literals*
(`request::endpoint_variables`) and substitutes a tenant's value into literals only, precisely so
nothing a caller passes can be substituted into. So the pin resolves end to end with no change to the
runtime port, and no tenant id is committed here — the story's own constraint.

**`ConfigField` stays connector-scoped (C-177's question), and the reason is the runtime port.** A
value is addressed by `(tenant, provider, service, kind, name)` where `name` is the **binding
target**, not the field name (C-197) — so the service already keeps Contentful's two spaces apart,
and a per-service field namespace would buy shorter form labels at the cost of renaming every shipped
field a host has stored a value under. A field that legitimately spans services has no answer under
it either: `service` is exactly one, always concrete, so "spans services" is two fields today and
would still be two fields after. Recorded in `config.rs`'s module docs. What *did* come out of C-177
is a new refusal in the same family: two fields of one service that would resolve the same
placeholder are refused (`two_fields_that_would_share_one_placeholder_are_refused`), because the
module carries one placeholder per pinned value and a host would key them to one slot — which is
exactly the C-197 collapse that sent a management write into whichever space the delivery reads had
been configured with.

**The two motivating providers were revisited, not excused.**

| | before | after |
|---|---|---|
| `cloudflare` | `zone_id` a required `params.path` argument on 4 of 5 operations | `binds = "path.zone_id"`, a parameter of none; `cloudflare-zone-list` stays unscoped because it is the call that *discovers* the value |
| `vercel` | `teamId` an optional `params.query` argument on all 5 | `binds = "query.teamId"`, mandatory, sent on all 5, a parameter of none |

Both files' header comments were rewritten rather than annotated — each had argued at length that the
argument shape was the only one the schema could express, and Cloudflare's said in as many words that
its five operations "would drop the parameter in favour of it" if `[[config]]` ever reached a path.
Vercel's five operation `description`s were rewritten too: they told a model that omitting `teamId`
was dangerous, which is no longer true and no longer possible.

**Algolia (C-164) is unblocked by one of its three findings, not all three.** A non-secret value now
reaches a header without a false `secret = true`, and
`a_pinned_header_reaches_the_emitted_request_and_not_the_signature` proves it end to end against the
emitter. But `binds` names exactly *one* destination, and the hostname and the header carry different
placeholders (`app_id` vs `X-Algolia-Application-Id`), so the same value still cannot be declared once
and reach both positions — an operator would type the application id twice with nothing keeping the
two in step. That third finding is asserted rather than glossed
(`the_hostname_and_the_header_are_still_two_declared_fields_with_two_slots`), and
`providers/algolia.toml` is still not shipped.

**Gate.** `cargo fmt --all --check`, `cargo build --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` are clean.
`cargo test --workspace --no-fail-fast` leaves **four** red, all of them the whole-catalogue
staleness AGENTS.md tabulates and fences to the coordinator —
`the_committed_tree_is_a_fixed_point_of_a_build`,
`a_build_plans_both_readme_images_and_they_are_current`,
`the_build_writes_and_checks_site_catalog_json` and
`every_shipped_operation_carries_its_metadata_and_its_flux`. The last is the same `catalog.json`
staleness as the third, reached from the per-operation check rather than the document-level one:
this story changed operation descriptions and emitted Flux on two providers. Per-provider artifacts
are committed and are a fixed point (`build --provider <id>` twice, no diff); `diff` over the full
catalogue reports no drift in any *other* provider's module, which is the acceptance item about
byte-identity.

**What a follow-on should pick up.** `Position::validate_value` is a predicate this repository
provides and only the loader currently calls, against `example`. The host-side call site is
`connector-pack`'s `Build::endpoints`/`request::build`, which substitutes the real value — wiring it
there is what turns "a pinned path segment cannot escape its segment" from a checked *declaration*
into a checked *request*, and it was out of this story's write set. Related: `connector-pack`'s
`Field::Endpoint` is now the kind every pinned value resolves through, so its doc-comment ("a `{var}`
in a service's `base_url`") understates what it carries.
