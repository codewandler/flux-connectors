---
id: C-187
title: "`[[config]]` can pin a base URL and nothing else, so an operator cannot scope a connector to one tenant"
pillar: Spec
status: ready
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

- [ ] A non-secret, operator-supplied value can reach a request header. Note the shape of the problem:
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

- [ ] Say whether `ConfigField` should be a per-service namespace like operations and channels are, and
      if so what happens to a field that legitimately spans services.

## Acceptance

- [ ] A `[[config]]` field can bind a value that reaches a path segment and/or a query parameter, not
      only `base_url`. Decide which of the two you support and **record why** — they are not equally
      safe, and a query binding that silently changes a write's target account is the harder case.
- [ ] **An operator-pinned value must not remain a caller argument.** If a value is pinned at install
      time, the emitted operation should not also accept it — otherwise the pin is advisory and a model
      can override the operator, which is the opposite of the point. Assert this.
- [ ] The interaction with `Level` is stated: the configuration contract says *operator* level is
      derived, never authored, so pinning a tenant value has to land on the right level by derivation
      rather than by declaration.
- [ ] **Failing-first test:** a provider binding a config field to a path segment does not load today.
- [ ] `providers/cloudflare.toml` and `providers/vercel.toml` are revisited, or this story records why
      they stay as they are. They are the two motivating cases.
- [ ] Every other provider's emitted module is byte-identical.

## Notes

- Read both connectors' `## Progress` notes first — each records the gap at the point it was hit.
- Do **not** solve this by letting a provider write a literal into a path template. A pinned value is
  operator data supplied at install time, not connector data known at compile time; conflating them
  would put a tenant id in the repository, which is the same category error as a credential value.
- Worth checking while here: whether a **header** can be operator-pinned. `const_headers` (C-55) pins a
  header the *connector* knows; nothing pins a header the *operator* knows, and an
  `X-Account-Id`-shaped vendor would want exactly that.
