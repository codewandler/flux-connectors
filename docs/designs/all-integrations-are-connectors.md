# Design: all integrations are connectors

**Status:** accepted direction, owner-confirmed 2026-08-03 · **Scope:** cross-repository migration

The cross-family source is `../flux/docs/designs/ecosystem.md`. This record applies that decision to
this repository: the old split between generated HTTP connectors and hand-written technology
plugins is superseded. HTTP, a local executable, a container, a guarded socket, the stdio plugin
protocol and a remote Exchange binding are runtime choices behind one connector surface.

## Destination

Every official external integration is represented in the connector catalogue. Its operations,
events, configuration, credential requirements, effects and runtime are reviewable without starting
it. Integration-specific declarations and code live in this repository; Flux knows runtime **kinds**
and guarded mechanisms, never vendors. Exchange may host the same connector for a tenant, but local
Flux remains a complete path and never requires that service.

The ownership split is:

| Repository | Owns |
|---|---|
| `flux` | Flux-Lang, authorization and approval, guarded IO, generic runtime protocols and the local connector host |
| `flux-connectors` | Connector declarations, generated modules, catalogue facts, and vendor-specific runtime artifacts |
| `flux-exchange` | Tenant credentials and settings, grants, runtime placement, isolated workers, remote invoke/subscribe, streams and leases |

A Rust adapter is not disqualified from being a connector. It is a connector whose runtime artifact
is hand-written because the protocol or lifecycle cannot be generated safely. “Compiled, never
interpreted” still holds: the connector source produces a reviewed bundle; a host does not invent
behavior by interpreting provider TOML at runtime.

## Bundle and runtime seam still missing

The catalogue already publishes `http`, `socket`, `process`, `container`, `plugin` and `remote`, but a
non-HTTP value currently describes only a kind. A complete bundle also needs:

- an operation-to-runtime binding, so `docker.container.list` names the adapter operation without
  pretending it is `http.request`;
- an immutable artifact identity for a binary or image, including supported platforms, digest,
  provenance and compatibility with the runtime protocol;
- declared capabilities and lifecycle: one-shot, streamed, subscribed or leased;
- a host-neutral result/error/stream contract shared by local Flux and Exchange;
- no credential value in the bundle or caller input. A host resolves the declared reference inside
  the selected trust boundary.

`connector-pack` remains zero-IO. It projects a declared connector into an executable plan; Flux or
Exchange supplies the runtime implementation. Adding a second vendor request builder to either host
would recreate the proxy the family rejected.

## Local and hosted are two placements of one connector

Local Flux loads the connector bundle and executes its declared runtime through `flux-system`. A
hosted caller sends an operation id and arguments to Exchange; Exchange derives the tenant, checks
the grant, resolves authority and dispatches the same bundle. Long-lived inbound events, process
stdout, socket read loops and lease liveness share the authenticated connector WebSocket.

Locally executing runtimes are never shared between tenants in one process. Exchange either runs
them in single-tenant mode or delegates to a per-tenant OS/container/pod boundary through `remote`.
That rule is placement policy, not a reason to keep an integration out of the connector catalogue.

## Measured migration inventory

Measured on 2026-08-03 with:

```bash
find ../flux/plugins -mindepth 2 -maxdepth 2 -name Cargo.toml -printf '%h\n' | sort
```

After excluding `host-kit` and `pack-index`, which are support/distribution crates rather than
integrations, the tree contains eighteen official adapters. Each has exactly one connector migration
owner:

| Current Flux adapter | Connector story |
|---|---|
| `confluence`, `gitlab`, `jira`, `slack` | C-499 collaboration migration |
| `docker`, `kubernetes` | C-500 infrastructure migration |
| `alertmanager`, `grafana`, `loki`, `opsgenie`, `prometheus` | C-501 observability migration |
| `onepassword`, `sql`, `vault` | C-502 data and secret migration |
| `aws`, `homer`, `huggingface`, `websearch` | C-503 remaining-adapter migration |

A migration is complete only when the connector preserves the supported operation/event surface,
declared effects and credential boundary; passes the same conformance suite locally and through
Exchange where its runtime is hostable; and the corresponding integration-specific Flux crate can
be removed. The stdio plugin protocol may survive as a generic runtime. The official native adapter
inventory may not.

## Program map

- **This repository:** C-495…C-505 own documentation, connector declarations, runtime bindings, runtime
  artifacts, migration waves and the completeness gate. C-405's runtime vocabulary and C-489…C-492's
  generated WebSocket channel work are delivered prerequisites; the active C-494 worktree supplies
  instance-aware host ports; C-47 is the SQL design input.
- **Flux:** C-500…C-506 own documentation, the local runtime host, Exchange client, locality conformance,
  plugin retirement and support-crate disposition. Existing C-394/C-397/C-399/C-435, D-215 and
  D-220 are reused rather than duplicated; C-493…C-499 are already consumed by pending maintenance
  and release worktrees.
- **Exchange:** X-111…X-120 own documentation, the stable remote protocol, generic runtime dispatch,
  single-tenant execution, multi-tenant isolation, streaming, leases, artifact trust and journeys.
  X-101…X-105 are the delivered inbound-WebSocket foundation.

## Cutover rule

No plugin is deleted on catalogue presence alone. For each row above:

1. freeze the existing plugin surface and behavioral fixtures;
2. make the connector pass those fixtures, including refusals and streaming behavior;
3. prove the local and hosted placements agree where both apply;
4. publish the connector/runtime artifact and migrate documentation and examples;
5. remove the Flux integration crate and its pack-index entry in the same release train, retaining a
   clear upgrade path rather than two indefinitely supported implementations.
