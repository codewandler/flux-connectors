---
id: C-529
title: "One deployment asks its origin question once"
pillar: Spec
status: done
priority: 0
design: docs/designs/connector-configuration.md
epic: connector-config
areas: [connector-spec, catalog, connector-cli, connector-pack, tests]
note: "a self-managed GitLab serves its REST API and its OAuth endpoints from one server; without a shared endpoint slot the connector must ask the operator the same question twice, and two slots that must agree and are not forced to is how a token exchange reaches a host the API never approved"
---

# One deployment asks its origin question once

## Goal

Let one `[[config]]` field fill the base-URL placeholder of several services, so a vendor whose
surfaces share a deployment asks its operator one question and stores one value under one approval.

A self-managed GitLab serves its REST API at `{origin}/api/v4` and its OAuth2 authorize and token
endpoints at `{origin}` — one server, therefore one fact. But a configuration value is addressed by
`(tenant, provider, service, kind, name)` and the two surfaces are different *services*, so without
this the connector must declare the origin twice.

**That is a security defect, not a redundancy.** Two slots that must agree and are not forced to is
how a token exchange ends up pointed at a host the API never approved — and the token endpoint is the
one destination that receives the client secret and returns credentials. C-508 already made this
origin `approval = "operator"`; two fields means two approvals that can disagree.

## Acceptance

- [x] `ConfigField::also_services` names further services whose base-URL placeholder this one value
      fills. `service` stays the head and remains the address the value is stored under.
- [x] Four refusals at the loader: only an `endpoint.` binding may be shared; every named service is
      declared; the head is not repeated; no service is named twice.
- [x] `Connector::config_filling` is the "can this service's URL be composed" lookup, beside
      `config_of`'s "which values does this service own". The loader's coverage check and the
      manifest emitter both use it.
- [x] A per-service manifest carries the field that fills its placeholder, so a host reading one
      manifest never sees a variable nothing declares. Each field keeps its own `service`, so the
      shared slot reads as one address rather than two.
- [x] `catalog::ConfigField::also_services` publishes it, because a host composing a sibling
      service's URL must consult it or report an unbound placeholder for a value already supplied.
- [x] Sharing is **stated, never inferred.** Contentful's `delivery_space_id` and
      `management_space_id` both bind `endpoint.space_id` in different services and stay two values;
      keyed as one, a management write went to whichever space the delivery reads had been
      configured with — a `200` from a real server rather than a refusal.
- [x] Failing-first tests: the shared slot loads, the unshared control is refused, and each of the
      four refusals fires against the field that caused it.
- [x] The published `provider-toml.schema.json` documents the field.

## Progress

- 2026-08-11: Implemented, and consumed immediately by [C-530](C-530-gitlab-delegated-oauth.md).

## Notes

The alternative was an `origin` field on `OAuth2Spec` — smaller, and wrong. It would put a
*destination* in a second spellable place, which is exactly the defect
[C-523](C-523-publish-canonical-https-origin.md) exists to remove: *"a third copy would let one
spelling be approved while another is executed."* Keeping `OAuth2Spec.endpoint` a **reference to a
declared endpoint** is what holds the token exchange inside the egress allow-list by construction,
since `http_hosts` derives from declared base URLs. A field holding a URL is a field that can hold a
URL somebody chose.

`crates/connector-pack/tests/request.rs` still hand-parses provider TOML and had to learn the rule
too. Its own comment says that reader should have been deleted when C-87 put configuration in the
catalogue; it is now one change further behind.
