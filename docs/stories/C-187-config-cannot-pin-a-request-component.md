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
