# Design: all integrations are connectors

**Status:** accepted direction, amended by C-507 on 2026-08-03 · **Scope:** cross-repository migration

The cross-family source of truth is
`../flux-roadmap/decisions/0001-exchange-executes-official-integrations.md`. This record applies that
decision to this repository: the old split between generated HTTP connectors and hand-written
technology plugins is superseded, as is the later proposal to execute official connectors in both
Flux and Exchange. HTTP, a connector-owned executable, a container, a guarded socket and the framed
stdio protocol are runtime choices behind one connector surface executed by Exchange.

## Destination

Every official external integration is represented in the connector catalogue. Its operations,
events, configuration, credential requirements, effects and runtime are reviewable without starting
it. Integration-specific declarations and code live in this repository; Flux knows no vendor.
Exchange is the sole supported execution boundary for every official external integration. Flux
embeds one native Exchange client, holds only a Service Account token and has no local vendor
adapter, plugin or connector-runtime fallback.

The ownership split is:

| Repository | Owns |
|---|---|
| `flux` | Flux-Lang, agent loop, model-facing tool projection, authorization, approval and the embedded Exchange client |
| `flux-connectors` | Connector declarations, generated modules, catalogue facts, runtime plans and vendor-specific runtime artifacts |
| `flux-exchange` | Tenant credentials and settings, grants, all official integration execution, runtime placement, isolated workers, invoke/subscribe, streams, leases and audit |

A Rust adapter is not disqualified from being a connector. It is a connector whose runtime artifact
is hand-written because the protocol or lifecycle cannot be generated safely. “Compiled, never
interpreted” still holds: the connector source produces a reviewed bundle; Exchange does not invent
behavior by interpreting provider TOML at runtime.

## Bundle and runtime seam still missing

The catalogue already publishes `http`, `socket`, `process`, `container`, `plugin` and `remote`, but a
non-HTTP value currently describes only a kind. A complete bundle also needs:

- an operation-to-runtime binding, so `docker.container.list` names the adapter operation without
  pretending it is `http.request`;
- an immutable artifact identity for a binary or image, including supported platforms, digest,
  provenance and compatibility with the runtime protocol;
- declared capabilities and lifecycle: one-shot, streamed, subscribed or leased;
- a result/error/stream contract consumed by Exchange and projected through its authenticated API;
- no credential value in the bundle or caller input. Exchange resolves the declared reference
  inside the tenant-derived trust boundary.

`connector-pack` remains zero-IO. It projects a declared connector into an executable plan; Exchange
supplies the runtime implementation. Flux consumes only Exchange's authenticated catalogue and
operation protocol. Adding another vendor request builder to Exchange or any such path to Flux would
recreate the proxy and duplicate execution path the family rejected.

## Exchange is the one official execution placement

Flux sends an operation id and arguments through its embedded client. Exchange derives the tenant
from the authenticated Service Account, checks the grant, resolves authority and dispatches the
connector bundle without returning a vendor credential. Long-lived inbound events, process stdout,
socket read loops and lease liveness share the authenticated connector WebSocket.

For local CLI use, Exchange itself may run on the same machine in single-tenant mode. A shared
Exchange delegates locally executing runtime plans to a per-tenant OS/container/pod boundary and
refuses them when that isolation is absent. Neither topology moves the execution boundary into Flux.
The `connectors-api` binary remains a reference/development host for existing HTTP seams, not a
supported official integration placement.

## Measured migration inventory

Re-measured on 2026-08-03 with:

```bash
find ../flux/plugins -mindepth 2 -maxdepth 2 -name Cargo.toml -printf '%h\n' | sort
```

The command prints twenty manifests. After excluding `host-kit` and `pack-index`, which are
support/distribution crates rather than integrations, the tree contains eighteen official adapters.
Each has exactly one connector migration owner:

| Current Flux adapter | Connector story |
|---|---|
| `confluence`, `gitlab`, `jira`, `slack` | C-499 collaboration migration |
| `docker`, `kubernetes` | C-500 infrastructure migration |
| `alertmanager`, `grafana`, `loki`, `opsgenie`, `prometheus` | C-501 observability migration |
| `onepassword`, `sql`, `vault` | C-502 data and secret migration |
| `aws`, `homer`, `huggingface`, `websearch` | C-503 remaining-adapter migration |

C-505 replaced this prose census with the retained source
[`migration/native-plugins.toml`](../../migration/native-plugins.toml) and the offline executable
check:

```bash
cargo run -p connector-cli -- migration-check --flux-root ../flux
```

The check reads the supplied Flux workspace and member manifests, classifies `host-kit` and
`pack-index` separately, and fails on an unaccounted integration. Inventory rows survive legacy
deletion, so a removed crate remains visible rather than disappearing from the population being
checked. The reusable evidence and publication workflow is
[`migration/README.md`](../../migration/README.md).

A migration is complete only when the connector preserves the supported operation/event surface,
declared effects and credential boundary; passes frozen legacy-plugin-versus-Exchange conformance;
and the corresponding integration-specific Flux crate can be removed. A framed stdio protocol may
survive behind Exchange as a connector-owned runtime artifact, but neither it nor an official
adapter remains a Flux release artifact.

## Program map

- **This repository:** C-495…C-505 own connector declarations, runtime bindings, runtime artifacts,
  migration waves and the conformance ratchet; C-507 records adoption of the cross-repository
  decision. C-405's runtime vocabulary and C-489…C-492's generated WebSocket channel work are
  delivered prerequisites; C-494 supplies instance-aware host ports; C-47 is the SQL design input.
- **Flux:** C-500…C-506 own the embedded Exchange client, legacy-plugin-versus-Exchange cutover
  fixtures, incremental adapter retirement and unconditional removal of the official plugin release
  pipeline. Flux owns no official connector runtime host or local fallback.
- **Exchange:** X-111…X-120 own the stable remote protocol, generic runtime dispatch, single-tenant
  execution, multi-tenant isolation, streaming, leases, artifact trust and journeys. X-101…X-105
  are the delivered inbound-WebSocket foundation.

## Cutover rule

No plugin is deleted on catalogue presence alone. C-505 first establishes the complete inventory,
frozen fixture format and release ratchet. Then, for each row above:

1. freeze the existing plugin surface and behavioral fixtures;
2. make the connector pass those fixtures through Exchange, including refusals and streaming behavior;
3. publish the connector/runtime artifact from the connector/Exchange pipeline and migrate
   documentation and examples;
4. remove the Flux integration crate and its pack-index entry in the same release train, retaining a
   clear upgrade path rather than two indefinitely supported implementations.

The waves land in fixed order: collaboration (C-499), infrastructure (C-500), observability (C-501),
data/secrets (C-502), then the remaining adapters (C-503). Each wave extends the C-505 ratchet and may
delete proven adapters incrementally; neither the harness nor deletion waits for one global cutover.
