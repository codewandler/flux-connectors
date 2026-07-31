---
id: C-197
title: "The configuration port has no service, so two services' variables of the same name collapse into one"
pillar: Bridge
status: ready
priority: 1
design: docs/designs/connector-configuration.md
areas: [bridge, connector-pack, connector-catalog]
note: "found by C-193's review, confirmed against contentful: `delivery_space_id` and `management_space_id` both bind `endpoint.space_id` under different services, and the runtime port keys on (tenant, provider, kind, name) with no service — so both resolve the same value. A tenant whose environments differ reads the WRONG environment and gets a 200, not a refusal. The Rust catalogue's `Operation` carries no `service` field, which is why C-193 could not fix it"
---

# The configuration port has no service, so two services' variables of the same name collapse into one

## Goal

Key a tenant's endpoint configuration by service as well as by provider, so that two services of one
provider binding the same variable name resolve to two different values — because the IR already
says they are two different things.

## What was measured

Found by the independent review of [C-193](C-193-templated-hosts-never-resolve.md), and confirmed in
the shipped catalogue rather than predicted.

`providers/contentful.toml` declares **two** configuration fields that bind the same placeholder
under **different services**:

| field | `binds` | service |
|---|---|---|
| `delivery_space_id` | `endpoint.space_id` | `delivery` |
| `management_space_id` | `endpoint.space_id` | `management` |

The provider file explains why they must be distinct. The runtime port does not have the vocabulary
to keep them apart: `crates/connector-pack/src/config.rs` keys on **`(tenant, provider, kind, name)`**
— no service — so contentful has exactly one `space_id` slot.

The consequence, traced end to end by the reviewer:

- `contentful-entry-get` — service `delivery`, host `cdn.contentful.com`
- `contentful-entry-create` — service `management`, host `api.contentful.com`

both resolve the **same** `space_id`. A tenant whose delivery and management environments differ
does not get a refusal. It gets a **`200` from the wrong environment, with a real management token**
— a write into a space the operator did not name, or a read of content they did not ask for.

**This is the same defect as [C-194](C-194-service-selection-leaks-config-graphs-verify.md) seen
from the other end.** C-194 found service selection leaking `config` *across* services at the seam;
this is the runtime port unable to *tell them apart* at all. The IR can express a distinction two
consumers cannot.

## Why C-193 could not fix it

Not an oversight — a schema boundary, and worth stating so nobody re-litigates it:

`crate::Operation` in `crates/catalog/src/generated/*.rs` **carries no `service` field.**
`web/public/catalog.json` does; the embedded Rust catalogue does not. So keying the port by service
requires adding a field to the `connector-catalog` crate's public type and regenerating every
provider's table — which moves artifacts, and would have broken C-193's own acceptance that no
artifact moved.

## Acceptance

- [ ] **Failing-first test:** two operations of one provider, in different services, binding the same
      variable name, resolve to **different** values. Contentful is the real case; assert against the
      shipped catalogue rather than only a fixture, so the test cannot pass while contentful stays
      broken.
- [ ] `catalog::Operation` carries its `service`, and the generated tables carry it. Expect **every**
      provider's `generated/*.rs` and `catalog.json` to move; that is the point, not a regression.
      This is a **breaking change to `connector-catalog`'s public type** — see the sequencing note.
- [ ] The port keys on `(tenant, provider, service, kind, name)`, and `Field::Endpoint`'s doc says so.
      It currently reads *"a `{var}` in a service's `base_url`"* while the key has no service in it,
      which is the sentence that made this invisible.
- [ ] A provider declaring two same-named bindings in two services is exercised, not merely
      supported — contentful is that provider today.
- [ ] The scoped gate is green and the build is a fixed point after regeneration.

## Progress

- (not started)

## Notes

- **Sequencing matters more than usual.** This changes `connector-catalog`'s public type, and
  [C-190](C-190-publish-catalog-pack-secrets.md) wants that crate on crates.io. Doing this *after*
  publishing burns a major version on the first release; doing it *before* costs nothing. It should
  therefore land **before the first publish** — see [C-195](C-195-crates-io-release-workflow.md).
  This is the strongest argument yet for not publishing `connector-catalog` early after all, and
  that trade should be settled explicitly rather than by whichever lands first.
- **Check C-87 before starting.** It publishes the configuration surface and owns the `service` field
  on `ConfigField`. If C-87 is close, these two want doing together — both touch how a service's
  configuration is identified, and doing them apart means regenerating every artifact twice.
- Contentful is harmless *today* only because one space is used for both APIs. Nothing declares that,
  and nothing checks it.
