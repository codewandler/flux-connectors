# Design: the connector configuration surface

**Status:** accepted (IR, loader and consumer artifacts landed) · **Pillar:** Spec (+ Codegen, Bridge) ·
**Epic:** `connector-config` · **Stories:** C-86 … C-89 ·
**Supersedes:** [C-68](../stories/C-68-endpoint-binding.md)'s binding half ·
**Amends:** [C-22](../stories/C-22-auth-conformance-matrix.md), [C-62](../stories/C-62-codegen-subscription-ops.md), [C-67](../stories/C-67-required-scopes.md)

## Why

Every other part of this repository models **how a credential reaches the wire**. Nothing modelled
**how it gets there in the first place**.

That is not a cosmetic gap. Four shipped providers cannot produce a valid URL until a human supplies a
tenant value — `{subdomain}`, `{site}`, `{shop}`, `{domain}` — and each carried the same `SCHEMA GAP:`
comment saying nothing bound it. `catalog.json` published an `unbound-base-url-template` issue for
them: a diagnosis with no remedy attached, and one that named only the *first* variable because the
function behind it was called `first_template_variable`.

Meanwhile the only human-facing string in the model, `description`, is **already spoken for** — it is
the text a model receives as a tool contract — and it shows:

```toml
# providers/slack.toml, before this work — a label, a placeholder and a scope list in one sentence
description = "Slack bot user OAuth token (xoxb-…) with chat:write, channels:history, users:read and reactions:write"
```

A connector that describes eleven operations and cannot tell a product to ask for a subdomain is not
installable by anyone who has not read its source.

## The boundary

**This repository declares; flux resolves; a UI renders.** Nothing here holds a value, a URL, or a
callback address.

flux already owns resolution and must not be duplicated:

- `ConfigSpec { name, env, description }` and `EndpointSpec { name, env, http_hosts, default,
  template }`. **`EndpointSpec::template` already composes `https://api.atlassian.com/ex/jira/{cloud_id}`
  host-side** from `ConfigSpec` values, percent-encoded — a 1:1 fit for `{subdomain}.zendesk.com`. So a
  `ConfigField` names the *destination* rather than re-implementing the templating.
- **Secret vs non-secret is a type-level partition**, enforced host-side: `resolve_config` refuses to
  return a secret-classified env key through the non-secret `config` capability. Our `secret` flag
  must **agree** with that, which is why agreement is a loader rule and not a convention.
- Credential storage, redaction, OAuth grant execution, egress allow-lists. All flux's.

## Configuration has two levels

The finding that shapes the model, and neither repository had it:

| level | set by | how often | examples |
|---|---|---|---|
| **operator** | whoever runs the product | once per vendor | OAuth `client_id`, `client_secret` |
| **connection** | each end user | once per tenant | `{subdomain}`, a pasted token, the grant result |

Conflating them is a real defect in both directions: ask an end user for a client secret and the
product's own credential is in every customer's hands; hard-code a subdomain and the connector serves
exactly one of them.

**`Level` is derived from `binds`, never authored.** It is a consequence of where the value goes, and
an author who could state it could state it wrongly.

## Shape

```toml
[[config]]
name     = "subdomain"
label    = "Zendesk subdomain"
help     = "The part of your Zendesk URL before `.zendesk.com` — if you sign in at `acme.zendesk.com`, this is `acme`"
example  = "acme"
format   = "subdomain"
docs_url = "https://support.zendesk.com/..."
binds    = "endpoint.subdomain"
```

`binds` is a validated string, the same shape `Param::wire` and `HmacSpec::signed` already use:

| form | level | secret | meaning |
|---|---|---|---|
| `endpoint.<var>` | connection | no | a `{var}` in the service's base URL |
| `credential.<name>` | connection | **yes** | the secret half of an `AuthMethod` |
| `username.<name>` | connection | no | the username half of a `basic` credential |
| `oauth.client_id` | operator | no | the app registration |
| `oauth.client_secret` | operator | **yes** | ditto |

The username half is a **separate prefix, not a `.user` suffix**, because credential names contain
dots (`zendesk.api_token`) and a suffix would be ambiguous with a credential genuinely named `….user`.

### `format` is a closed enum, not a regex

A renderer given `^[a-z0-9][a-z0-9-]*$` can reject a value and cannot explain why. A renderer given
`Format::Subdomain` knows the rule, the message and the example. The enum also lets this crate check a
field against *itself*: `example` is validated against `format` at load, so a provider claiming
`format = "subdomain"` with `example = "https://acme.zendesk.com"` is refused rather than shipped as a
misleading placeholder a user would copy.

A free-form `pattern` escape hatch is deliberately **absent**. No shipped provider needs one, and it
would mean a `regex` dependency in a crate that has six. It lands when a real provider needs something
the enum cannot say.

### `choices` is a closed set of **values**, and it narrows `format` rather than replacing it (C-225)

`format` answers *what shape is this value*. `choices` answers *which values are legal*. Two vendors
measured the difference in one wave, and the failure mode is the expensive kind:

| vendor | the set | what a wrong answer does |
|---|---|---|
| `newrelic` | `api.newrelic.com`, `api.eu.newrelic.com` | `401` on every call, indistinguishable from a bad key |
| `intercom` | `api.intercom.io`, `api.eu.intercom.io`, `api.au.intercom.io` | the same, and the file recorded it as an open `SCHEMA GAP` from the day it shipped |

`format = "hostname"` accepts every syntactically valid host on the internet, so before this the
connector, the loader, any form built from it and this repository all accepted
`api.not-new-relic.example` without complaint. The operator's first move on a `401` is to rotate the
credential, which changes nothing, and no signal anywhere points at the host.

```toml
[[config]]
name    = "host"
label   = "New Relic API host"
help    = "Which region this New Relic account lives in…"
example = "api.newrelic.com"
format  = "hostname"
choices = [
  { value = "api.newrelic.com",    label = "United States" },
  { value = "api.eu.newrelic.com", label = "European Union" },
]
binds   = "endpoint.host"
```

**The label is mandatory**, which is why a choice is a table and not a string: an operator knows
their account is in Frankfurt and does not know that `api.eu.newrelic.com` is what that means.

**`format` stays, and stays load-bearing.** Collapsing the two is tempting and wrong in three
concrete ways: the format is what every choice is validated against at load (so a set cannot widen
the field past its own rule), it is the input type a renderer falls back to, and it is what an
`example` still answers to. `example` now answers to both — shape *and* membership.

**Deliberately not a constraint language.** A closed list of values with labels is the whole of it.
Ranges, patterns and conditionals are each their own argument, and the same call `format`'s missing
`pattern` already makes applies.

### `also_binds` is one question reaching more than one **destination** (C-229)

`choices` is about the set of legal *values*; this is about the set of *destinations*. They are
genuinely different questions, and Algolia is the vendor that forces the second one:
`X-Algolia-Application-Id` is a mandatory header on every call, and the *same* application id also
composes the request's hostname. The value is not a secret — Algolia publishes it in client-side code
— so it belongs in `[[config]]`, and until this landed there was no way to say it once.

C-164 refused to ship the connector twice rather than say it dishonestly, and measured all three ways
of faking it:

| shape | outcome |
|---|---|
| two fields, different names (`endpoint.app_id` + `header.X-Algolia-Application-Id`) | **loads** — and is the problem: two host-side slots, one answer, nothing keeping them in step, and no honest `help` for the second field |
| two fields, one name | **refused** — the shared-slot rule (invariant 11): *two questions that share an answer are one question* |
| one field, header pin alone, hostname resolving from it | **refused** — only `endpoint.<var>` binds a `base_url` variable (invariant 1) |

Both refusals are right. What was missing is **the one question**:

```toml
[[config]]
name       = "app_id"
label      = "Algolia application id"
help       = "…it forms the hostname every call goes to, and is sent as a header on every request — you supply it once here"
example    = "B1G2GM9NG0"
binds      = "endpoint.app_id"
also_binds = ["header.X-Algolia-Application-Id"]
```

One `name`, one `label`, one `help`, one row in a form, **one host-side slot**, two destinations.

**Why `also_binds` and not `binds` becoming a list.** A list of peers has no head, and this
declaration needs one, because `Position::name` is deliberately both the `{placeholder}` and the wire
spelling. A field whose destinations spell the value differently — `app_id` in the host,
`X-Algolia-Application-Id` on the wire — therefore forces a choice about which spelling the emitted
module carries. With a head there is one rule and no conditional: **the emitted module carries
`binds`' own target, everywhere**, and a further destination contributes only what the vendor sees.
With a bare list the answer would be "element zero" — a convention about ordering rather than a
property of the declaration. It also keeps `binding()`, `level()` and the stored `(kind, name)`
address exactly what they were for every field that existed before.

**A further destination is a request position and nothing else** (`path.`, `query.`, `header.`).
Every other kind resolves under its own address through a different port: a credential and an OAuth
half through the secret side, a `username.` under its own `(kind, name)`. One collected value has one
address, so a field naming any of those names it alone. An `endpoint.` destination is the head or
nothing — its spelling is fixed by a `base_url` the author already wrote, so it is the destination
with the least freedom and the natural head.

**Every destination validates, and the host rule is the strict one.** The `example` and every
`choice` are checked once per destination, because the intersection of two rules is taken by checking
both. That matters in one direction specifically: `acme.example@evil.example` passes the path, query
and header rules — none of those positions cares about an `@` — and substituted into an authority it
moves the origin. `connector-pack` reaches the same conclusion from the runtime end: a variable it
sees in two positions is held to every rule at once, and is not encoded differently per destination.

**A stored value that later leaves the set keeps working.** Membership is checked where a value is
*supplied* — `ConfigField::permits`, called by whatever accepts a value from a human, which in this
repository is `connectors-api`'s `PUT /v1/config/…` — and never where a stored value is read back
and substituted. A vendor adding a region must not brick a connection configured before it existed;
the next edit of that field is where the operator is asked to pick again. Refusing at read time would
turn a catalogue update into an outage on connections that were never wrong.

**The set is published**, in `connectors/<id>.connector.toml` as `[[config_choices]]`, in
`catalog.json` as `config_choices`, and in `catalog::Provider::config_choices`. That is *not* C-87
landing early: it is the set alone, addressed by `(service, kind, name)`, because a closed set a
renderer cannot see is a text box with extra steps. The rest of the surface — labels for every field,
help, `format`, `binds`, the derived level, `verify`, subscription and setup — now travels through
C-87's manifest and catalogue projections. C-87 also replaces the lossy `auth.oauth2` boolean with
the complete OAuth declaration and therefore moves `catalog.json` to schema version 3.

### Operator-approved origins are a connection field with an activation policy (C-508)

A self-managed product cannot declare a closed host set without pretending to know every
installation. It instead declares one complete HTTPS origin and the connector keeps ownership of
the API path:

```toml
[[config]]
name     = "origin"
format   = "origin"
default  = "https://gitlab.com"
approval = "operator"
binds    = "endpoint.origin"
```

`format = "origin"` accepts only an HTTPS scheme plus authority and optional effective port. It
accepts no userinfo, path, query or fragment, so a supplied value cannot replace the connector's
`/api/v4`. `approval = "operator"` is deliberately separate from `level`: the field is still
collected per connection, but a non-default proposal is inert until deployment/operator policy
approves and pins that exact connection resolution.

The configuration port returns the value and its approval as one instance-aware answer. Projection
freezes that answer once, then request composition and permission subjects read the same snapshot.
A pre-existing store that knows only values therefore treats every custom origin as unapproved;
there is no compatibility path that silently activates tenant-controlled authority.

The policy is declaration data, not a GitLab-only host feature. Manifest, embedded declaration JSON
and public catalogue carry `format`, `default`, `approval`, `binds` and derived `level`; renderers can
show the activation requirement without parsing provider TOML. Configured values never enter those
artifacts or a model-visible operation schema.

## Invariants — all refusals

1. **A connector asks for everything it needs.** Every `{var}` in every service base URL is bound by
   exactly one field. This is what closes the recorded gap; a variable nobody declares is a connector
   that cannot be configured and cannot say why.
2. **A connector asks for nothing it cannot use.** Endpoint, credential and OAuth references all
   resolve, or the field is refused.
3. **`secret` agrees with `binds`.** The rule with a security edge — see the boundary section.
4. **A username field is only for `basic`.** Every other scheme sends the secret alone, so the value
   would have nowhere to go.
5. **A field is renderable.** `label` and `help` are mandatory and non-empty; defaulting `label` to
   `name` would ship `zendesk.api_token` as user-facing copy.
6. **An example satisfies its own format.**
7. **A secret field declares no example at all** (C-231). Not a documentation preference: a
   token-shaped literal in a committed file has tripped GitHub push protection and blocked a release
   in this repository, and a placeholder that *is* a real token is a disclosed credential rather than
   a blocked push. It also buys nothing, because nobody recognises their own secret from an example
   of someone else's — the shape of the value goes in `help`, as prose. This is a **loader refusal**
   and not a test over `providers/`: the loader already checks `example` against `format` and against
   a pinned request position, so the "an example is documentation" objection was already answered by
   the code; and these crates are published, so the only form of the rule that reaches a downstream
   author writing their own provider TOML is one that fires at `provider::load`. The catalogue is
   covered as a consequence — `every_shipped_provider_loads` enumerates `providers/` from disk — so
   no per-connector restatement of it should exist. Scope: **secret fields only**; a non-secret
   field's placeholder stays welcome, and invariant 6 is the only rule it answers to.
8. **A verification operation is a read.** `verify` names the "Test connection" operation; a `high` or
   `destructive` one is refused, because a connection test runs unattended whenever someone opens a
   settings page.
9. **Config names join the shared member namespace** of their service.
10. **A closed set is checked against the field it narrows** (C-225). Every choice satisfies the
    field's own `format`; a set has at least two values, because a set of one is a constant and
    belongs in the base URL rather than in front of a human; every choice has a non-empty label and
    no value or label repeats, because each of those makes a form that cannot be answered; a `secret`
    declares none, which is invariant 7 in its stronger form (an example is one credential-shaped
    literal, a set is all of them); the `example` is one of the choices; and where the field pins a
    request position, every choice satisfies that position too — a permitted value that escaped its
    path segment would be a *sanctioned* way to address another resource.
11. **Two fields never share a slot, and never share a wire position** (C-197, extended by C-229). A
    host keys a value by `(tenant, provider, service, kind, name)` and the emitted module carries one
    `{placeholder}` per field, so two fields of one service whose slots collide are one slot — the
    collapse C-197 found between Contentful's two spaces, where a management write landed in whichever
    space the delivery reads were configured with. *Two questions that share an answer are one
    question.* C-229 does not weaken this; it answers the other half, and one field with two
    destinations is one question with one slot. The second clause is what a further destination makes
    newly possible: two fields, two slots, one header — a request carrying one of two values depending
    on an order nothing declares.
12. **A further destination is a request position, named once** (C-229). Every other kind resolves
    under its own address through a different port, so it cannot share a slot; and one value reaches a
    position once. Each destination's own rule applies to the `example` and to every choice, the host
    rule included — see the `also_binds` section for why the host rule is the strict one.

## Webhooks as a full exposure

The channel bindings epic gave a binding a `reply` and a `cursor`. It could not say how the binding
gets *registered*, so a product could show a callback URL and nothing else.

```toml
[channels.subscription]                        # vendors with a registration API
subscribe      = "acme-webhook-subscribe"
unsubscribe    = "acme-webhook-unsubscribe"
list           = "acme-webhook-list"
callback_param = "url"                          # which param takes OUR public URL

[channels.setup]                                # vendors without one — Slack
docs_url = "https://docs.slack.dev/apis/events-api/"
steps = ["Open your app at api.slack.com/apps", "…"]
```

**A `webhook` binding declares one or the other.** Same shape as the verification rule: a product that
knows a callback URL and nothing about what to do with it cannot finish an installation, and silence
is not one of the options.

Registration stays an **ordinary outbound write** — an authorized, approvable operation, never a
build-time side effect. The callback URL itself is still absent by design: it is the product's
deployment detail, and a connector carrying one would be describing someone else's infrastructure.

`EventDecl` also gains `default` and `group`, so a product can render a checkbox list. Slack's
`message` is the case: it fires for every human message in every channel the app is in, and until now
that warning existed only in the prose a *model* reads.

## The runtime port keys by service, because the declaration does (C-197)

The half above describes what a connector *declares*. `connector-pack`'s `ConfigStore` is what a host
*answers* with, and for one release the two disagreed about what identifies a field.

`ConfigField::service` is **exactly one, always concrete** — every declared field belongs to a
service, whatever it binds. But a field's `name` is unique across the *whole connector* rather than
per service, so a connector whose two services need the same `{variable}` declares it twice under two
names. `contentful` is the shipped case: `delivery_space_id` and `management_space_id`, both
`binds = "endpoint.space_id"`, in `delivery` and `management`.

The runtime port keyed on `(tenant, provider, kind, name)`, and the `binds` target is what it keys
`name` from — so those two fields collapsed into **one slot**. The failure is the bad kind:
`contentful-entry-create` on `api.contentful.com` resolved whatever `contentful-entry-get` on
`cdn.contentful.com` had, so a tenant whose two environments differ got a **`200` with a real
management token** — a write into a space nobody named. Nothing refused, because nothing was
missing.

The key is now **`(tenant, provider, service, kind, name)`**, and the service comes from
`catalog::Operation::service`, which C-197 added for it — the embedded catalogue catching up with
`catalog.json`, which had carried the service all along. Three consequences worth stating:

- **The reserved `default` is written out** in the catalogue and in the key. Elision is an *address*
  rule (`com.freshdesk.api:v2`); a consumer grouping by service needs a name in every row.
- **Every kind is keyed this way, `username.<name>` included**, because the IR declares every field
  under a service. A credential's *address* elides the service — it is declared at connector level —
  but the configuration field supplying its user half is not, and this port follows the declaration.
  A two-service connector with one `basic` credential binds the user half under each service that
  asks for it.
- **A service with nothing bound refuses by name** rather than borrowing its sibling's value, and the
  refusal quotes the service. Without that, an operator told `contentful` is missing
  `endpoint.space_id` has two fields answering to that description.

## What this does not settle

- **OAuth is unexercised.** `OAuth2Spec` is a landed type **no shipped provider uses**, so the
  operator level is proven only by a fixture. `tests/auth_archetypes.rs` asserts that gap rather than
  papering over it — the test fails the day a provider adopts OAuth, and the fix is to assert the form
  it generates. C-88.
- **The hosted redirect has no home.** `OAuthRedirect { port, path }` is loopback-only. A hosted
  callback is `https://app.example.com/oauth/callback`, supplied by the host — the connector should
  declare only that a redirect is required and any vendor constraint on it. C-89.
- **Codegen is no longer an open question.** Config and `verify` reach the manifest and
  `catalog.json`; subscription and setup travel with their channel binding; the derived level is
  published rather than authored. The complete `OAuth2Spec` replaces the old boolean in schema
  version 3, so a hosted product receives the paths, scopes, grants and redirect declaration it
  needs to start a grant.
- **Scopes.** A consent screen needs per-scope display text; C-67 designs bare strings. Its "union per
  service" is already the right primitive.
- **i18n.** Every string is English and inline, as everywhere else here.

## Open question, recorded rather than guessed

`Level` is derived with no authoring override. That is right for every case nameable today, but a
vendor with an operator-level *non-OAuth* value — a partner API key — would need one. The smallest
honest model derives it now and adds the override when a real provider forces it, which is the call
C-49 made about service tail segments.

## Alternatives considered

- **JSON Schema + a separate uiSchema.** Familiar (the repo already carries `JsonSchema` for params),
  but it splits one fact across two documents and invites them to disagree — and a JSON Schema cannot
  express `binds`, which is the part that makes a collected value go somewhere.
- **Presentation fields on `AuthMethod` only.** Smallest change, but a tenant value like `{subdomain}`
  is not a credential and would have had nowhere to live.
- **Overload `description`.** It is already the model-facing tool contract. One string serving two
  audiences serves each badly, which `providers/slack.toml` demonstrated before this work.
- **A second credential namespace for inbound/config secrets.** Splits the manifest's credential list,
  which is the one place an operator looks.
