---
id: C-225
title: "A configuration field cannot declare a closed set of values, so a two-choice region reads as free text and a wrong answer looks exactly like a bad key"
pillar: Spec
status: done
design: docs/designs/connector-configuration.md
epic:
areas: [connector-spec, bridge]
note: "found by the C-220 implementor 2026-07-31 and left unfiled deliberately because the board is coordinator-owned. Two vendors are already waiting on it: newrelic (2 regions) and intercom, whose providers/intercom.toml:123 records `the regional hosts are not selectable` as an open SCHEMA GAP"
---

# A configuration field cannot declare a closed set of values

## Goal

Let a connector declare that a configuration value comes from an enumerated set, so a renderer can
offer a choice and a wrong value is refused where it is entered rather than discovered as a 401.

## What was measured

`ConfigField` carries a `format`, and `Format` is a closed enum of value **shapes** —
`hostname`, `subdomain` and friends. It has no field for a set of permitted **values**, and the
struct is `deny_unknown_fields`, so a `values = [...]` declaration does not merely go unused: it
fails to load.

New Relic has exactly two API hosts (US and EU). The connector therefore ships:

```toml
base_url = "https://{host}/v2"
# host → endpoint.host, format = "hostname"
```

and the loader, any form built from it, and this repository all accept
`api.not-new-relic.example` without complaint. The C-220 contract test pins both halves against the
shipped file: an unrelated host still loads, and a `values = [...]` declaration is refused as an
unknown field.

## Why the failure mode is the expensive kind

**A wrong region returns `401` on every call, indistinguishable from a bad key.** The operator's
first move is to rotate the credential, which changes nothing, and there is no signal anywhere
pointing at the host. This is the same shape as [C-223](C-223-the-host-sends-no-user-agent.md) —
a status code that names the wrong cause — and the two will be diagnosed together or not at all.

## It is not a one-vendor expression

| vendor | evidence |
|---|---|
| `newrelic` | two hosts, US and EU, no pre-auth endpoint discloses which |
| `intercom` | `providers/intercom.toml:123` — *"SCHEMA GAP: the regional hosts are not selectable"*, with `api.eu.intercom.io` and `api.au.intercom.io` named at `:126-127` |

Intercom recorded this before New Relic existed. Two independent connectors reaching the same wall
is the signal that the IR is missing an expression, not that either connector was authored badly.

## The distinction worth preserving

C-220 found a counterexample to its own story's premise and recorded it rather than repeating the
premise: `providers/docusign.toml`'s `account_host` is *also* a vendor-owned host bound through
`endpoint.` with `format = "hostname"`. The real distinction is **discoverability** — DocuSign's
value is a field of the UserInfo response, so a wrong answer is a transcription error the operator
can check; New Relic's is a guess between two, with nothing pre-auth that discloses the right one.
A closed set helps most exactly where the value cannot be discovered.

## Acceptance

- [x] **Failing-first test:** a provider declares an enumerated set of values for a configuration
      field and it loads. It is refused today as an unknown field. Name the test.
      → `crates/connector-spec/tests/config_choices.rs::a_config_field_declares_a_closed_set_of_values_and_a_value_outside_it_is_refused`
- [x] Each permitted value can carry a human **label**, so a renderer shows "United States" rather
      than `api.newrelic.com`. A set of raw values is a dropdown nobody can read.
      → `Choice { value, label }`, `crates/connector-spec/src/config.rs`; both fields mandatory, and
      a blank or repeated label is a load refusal (`validate_choices`, `provider.rs`).
- [x] Settle whether a field with a closed set still needs a `format`, and record the reason. They
      answer different questions — shape versus membership — and collapsing them will be tempting.
      → **`format` stays.** Recorded in `config.rs`'s module docs, in `validate_choices`'s doc
      comment, and in the design's new §`choices` … narrows `format`. Three concrete reasons rather
      than a preference: every choice is validated against the format at load (so a set can never be
      *wider* than the field it narrows), the format is the input a renderer falls back to, and
      `example` now answers to both shape and membership.
- [x] A value outside the set is refused **at the point it is supplied**, and the refusal names the
      field and lists what is permitted. A refusal that says only "invalid" reproduces the diagnosis
      problem this story exists to remove.
      → `ConfigField::permits`, called by `connectors-api`'s `PUT /v1/config/…`
      (`crates/connectors-api/src/api.rs::put_config`). Asserted end to end by
      `crates/connectors-api/tests/config_choices.rs::a_host_outside_the_set_is_refused_and_the_refusal_names_the_answers`.
- [x] State what a host does with a **stored** value that later leaves the set — a vendor adding a
      region must not brick an existing connection. This is the half that is easy to skip and
      expensive to retrofit.
      → **Nothing.** Membership is checked on the write path only, never on read; stated in
      `config.rs`'s module docs, in `put_config`'s doc comment and in the design, and pinned by
      `a_stored_value_is_never_re_validated_on_the_way_out`.
- [x] `newrelic` and `intercom` both adopt it, and `intercom`'s `SCHEMA GAP` comment at
      `providers/intercom.toml:123` is removed rather than left describing a gap that closed.
      → both declare `choices` on a `host` field; intercom's `base_url` is now `https://{host}` and
      the gap comment is replaced by an account of the closure.
- [x] `crates/connectors-api/src/index.html` renders the choice as a choice. A closed set that still
      renders as a text box has moved the declaration without moving the benefit.
      → the configuration row's value control is a `<select>` of labels when the selected
      `(service, kind, field)` has a published set, and a text input otherwise.

## Notes

- Sequencing: this changes `connector-spec`'s public surface, so it runs solo or first in a wave —
  it collides with every provider story by definition.
- Do not let this become a general constraint language. A closed list of values with labels is the
  whole scope; ranges, patterns and conditionals are not this story and each would want its own
  argument.
- Related but distinct: [C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md) is about a
  value being *validated where it is substituted*. This is about the set of legal values being
  *declarable at all*. C-214 without this still cannot refuse `api.not-new-relic.example`, because
  nothing anywhere says what the legal hosts are.

## Progress

**2026-08-01 — landed on `impl/C-225`.** Every Acceptance item is ticked; the notes below are what a
reviewer or a resuming agent needs that the ticks do not say.

### The key is `choices`, not `values`

Each permitted value carries a label, so the entry is a table (`{ value = …, label = … }`) rather
than a string, and `values = [...]` would have been a misleading name for a list of tables. The
C-220 contract test probed `values`; it has been rewritten in place —
`newrelic_connector.rs::the_closed_set_of_two_hosts_is_declared_and_any_other_host_is_refused` is the
same claim inverted, and it now asserts the refusal rather than the gap.

### Six loader rules, all in `validate_choices`

Every choice satisfies the field's own `format`; a set has at least two values; every choice has a
non-empty label and no value or label repeats; a `secret` declares none (C-231's rule in its stronger
form — an example is one credential-shaped literal, a set is all of them); the `example` is one of
the choices; and a field that *pins a request position* has every choice checked against that
position, beside the `example` check it mirrors (`validate_pin`). That last one is the `binds`
interaction the story asked about: a permitted value that escaped its path segment would be a
**sanctioned** way to address another resource on the same host with the same credential.

`choices = []` is deliberately *not* a separate refusal: serde's `default` makes an empty list and an
absent key the same IR, and distinguishing them would mean an `Option<Vec<_>>` in the public surface
to carry a diagnostic nobody has needed.

### Publishing: the set, and only the set

The choices reach `connectors/<id>.connector.toml` as `[[config_choices]]`, `catalog.json` as
`config_choices`, `catalog::Provider::config_choices`, and the connect API's `ConnectorView`. Each
row is keyed by `(service, kind, name)` — the address `connector-pack`'s configuration port already
stores a value under, and the same segments `PUT /v1/config/<provider>/<service>/<kind>/<field>`
takes — so a consumer joins on the route it is about to call.

**This is not C-87 landing early.** Labels for every field, help, `format`, `binds`, the derived
level, `verify`, subscription and setup are still unpublished, and so is the breaking `auth.oauth2`
flattening C-87 has to settle. What is here is the part a closed set is worthless without: a set a
renderer cannot see is a text box with extra steps. C-87 folds these rows into its per-field block;
the row shape was chosen so that is an extension rather than a merge of two disagreeing lists.

`catalog.json`'s `schema_version` stays `2` — the key is additive, which is exactly the rule
`docs/designs/catalog-json.md` §Versioning states, and the design now documents the two new objects.

### Intercom's adoption is a behaviour change, and it is the deliberate one

`providers/intercom.toml` shipped US-only with a literal `base_url = "https://api.intercom.io"`, and
recorded "an EU workspace needs a second connector" as the remedy. It is now
`base_url = "https://{host}"` with a three-value set, so:

- **an EU or AU workspace can be connected at all**, which it could not before;
- **`host` is a required configuration value where none was required before.** An operator who
  previously needed only a token must now pick a region. `connector-pack` fails closed
  (`Error::MissingConfig`) rather than defaulting, so an existing deployment that upgrades without
  binding `host` gets a refusal naming the field — not a request to the wrong region;
- **the egress claim moved from one literal host to three enumerated ones.** That is narrower than
  the tempting alternative (a widened host list) in the way that matters: the reachable set is still
  enumerable from the artifacts. `crates/connector-cli/tests/shipped_providers_build.rs` was rewritten
  around that claim rather than deleted.

### What was deliberately not done

- **`hosts` in `catalog.json` still publishes `{host}` for a templated base URL**, even where the
  closed set makes the reachable hosts knowable. Narrowing the published egress list to the set is a
  real improvement and a separate change: it touches `catalog::host_of`, the per-service `hosts`
  claim and every consumer of it, and it wants its own argument about whether a template with a set
  should publish the template, the set, or both.
- **`connector-pack` does not consult the set.** That is the stored-value rule above, not an
  omission: the pack is the read path.
