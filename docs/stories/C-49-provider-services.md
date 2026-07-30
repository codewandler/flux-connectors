---
id: C-49
title: Model a provider's services as the middle addressing level
pillar: Spec
status: ready
priority: 4
design: docs/designs/provider-services.md
epic: connectors-v1
areas: [connector-spec, connector-flux, connector-cli]
note: provider → service → operations · one service per operation · unset means `default`
---

# Model a provider's services as the middle addressing level

## Goal
Give a provider an explicit **service** level between the connector and its operations — `s3` and
`bedrock-runtime` under AWS, `support` under Zendesk — so that a service is the unit you address,
version, select and install, and every operation belongs to exactly one of them. This is the "scope"
or "group" C-37 sketched as a bare path segment, promoted to a named thing with an owner.

## Acceptance
- [ ] **`Service` is an IR level, not a tag on the operation.** `Connector` gains
      `services: Vec<Service>`; `Service` carries `name`, `description`, an optional `base_url`
      override and an optional `api_version`. `Operation` gains `service: String`. A free-form
      `tags` field is explicitly rejected in the design: a tag cannot partition, version or host.
- [ ] **Exactly one service per operation, and services partition the operation set.** A property
      test asserts the per-service operation sets are pairwise disjoint and their union is every
      operation — the invariant that makes "install the whole s3 service" a well-defined set.
- [ ] **`service` unset means `"default"`.** The name is reserved: no `[[services]]` entry may
      declare it, and an operation naming a service that no `[[services]]` entry declares is a loud
      error listing the services that do exist — following C-3's treatment of duplicate op ids.
- [ ] **Byte-identical output for today's three providers**, all of which are single-service and
      therefore all-`default`. Failing-first: a test pinning the four goldens in
      `crates/connector-flux/tests/golden/` and the generated `.flux`/`.connector.toml` artifacts as
      unchanged by this story.
- [ ] **The service is the first path segment of C-37's gid, and `default` is elided from it.**
      `com.amazonaws/s3:2006-03-01#object-get` · `com.zendesk.api/support/tickets:v2#show` ·
      `com.freshdesk.api/tickets:v2#create` (default elided, so C-37's variable depth still holds and
      `default` never reaches a published address). `parse(render(x)) == x` round-trips including the
      elision.
- [ ] **`api_version` belongs to the service**, with the connector-level value as its default. AWS
      versions each service on its own date (`s3:2006-03-01`, `bedrock-runtime:2023-09-30`), so a
      single connector-level version cannot describe a multi-service provider.
- [ ] **The emitted unit is the service.** A provider with named services emits
      `<provider>-<service>.flux` plus `<provider>-<service>.connector.toml` per service; a
      `default`-only provider still emits `<provider>.flux` exactly as today. `http_hosts` in each
      manifest derives from that service's own `base_url` and is never widened to `*` (C-10).
- [ ] **Building can select one whole service** — every operation belonging to it and nothing else —
      by service name or gid; an unknown service is a loud error naming the available ones. A test
      selects one service from a two-service fixture and asserts the other's operations are absent.
- [ ] **Service fields land inside `HashDomain::of`** — they are part of a connector's compiled
      meaning, like C-37's addresses and unlike C-7's provenance. C-2's determinism tests stay green
      unchanged.
- [ ] `docs/designs/provider-services.md` records the decisions; `docs/designs/global-addressing.md`
      gets an amendment note pointing at it, since this story fixes the meaning of its middle level.
      `AGENTS.md` records the partition invariant beside the auth conventions.

## Progress
- Not started. Filed from a user request on 2026-07-30 that named AWS (`aws` = provider,
  `s3`/`bedrock` = services) as the motivating case.

## Notes
- **Sequence this before C-37, or land the two together.** C-37 is `ready` at priority 5 and adds
  `Operation.path: Vec<String>` as anonymous hierarchy. If it lands first, this story reshapes those
  fields immediately and the address scheme is published twice — and C-37's own stability contract
  ("an oip, once published, is never reused") makes the second reshape expensive. That is why this
  sits at priority 4.
- **Why one service per operation rather than a set.** A set makes the gid ambiguous (which segment
  renders?) and makes selection non-partitioning, so "add s3" could no longer be answered by set
  membership. If an operation genuinely serves two services, it is duplicated deliberately with two
  ids — which is visible — rather than resolved by a rule nobody can see.
- **This is the slicing unit a 163-operation provider needs.** C-18 curated babelforce down to a
  handful precisely because a provider is not a usable tool catalogue; services make the cut
  structural instead of editorial, and give C-41's bundle layout its per-service directory.
- Write back to C-42's `catalog.json` schema and C-44's explorer: both currently group by provider,
  and service is the grouping a consumer wants. The schema must carry the service even if the UI
  follows later.
- The first multi-service provider, and the AWS-specific gaps it surfaces, are
  [C-50](C-50-aws-services.md).
