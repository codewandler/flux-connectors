---
id: C-225
title: "A configuration field cannot declare a closed set of values, so a two-choice region reads as free text and a wrong answer looks exactly like a bad key"
pillar: Spec
status: ready
priority: 2
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

- [ ] **Failing-first test:** a provider declares an enumerated set of values for a configuration
      field and it loads. It is refused today as an unknown field. Name the test.
- [ ] Each permitted value can carry a human **label**, so a renderer shows "United States" rather
      than `api.newrelic.com`. A set of raw values is a dropdown nobody can read.
- [ ] Settle whether a field with a closed set still needs a `format`, and record the reason. They
      answer different questions — shape versus membership — and collapsing them will be tempting.
- [ ] A value outside the set is refused **at the point it is supplied**, and the refusal names the
      field and lists what is permitted. A refusal that says only "invalid" reproduces the diagnosis
      problem this story exists to remove.
- [ ] State what a host does with a **stored** value that later leaves the set — a vendor adding a
      region must not brick an existing connection. This is the half that is easy to skip and
      expensive to retrofit.
- [ ] `newrelic` and `intercom` both adopt it, and `intercom`'s `SCHEMA GAP` comment at
      `providers/intercom.toml:123` is removed rather than left describing a gap that closed.
- [ ] `crates/connectors-api/src/index.html` renders the choice as a choice. A closed set that still
      renders as a text box has moved the declaration without moving the benefit.

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
