# Design: the connector configuration surface

**Status:** accepted (IR + loader landed) · **Pillar:** Spec (+ Codegen, Bridge) ·
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
- **Codegen.** Config, `verify`, subscription and setup are in the IR and in the hash domain and reach
  no artifact yet. C-87 — which must also settle a **breaking** change: `site.rs` flattens the whole
  `OAuth2Spec` to `oauth2: bool`, so a hosted product cannot build an authorize URL at all.
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
