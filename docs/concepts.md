# Connector domain concepts

This page defines the vendor facts flux-connectors owns and the runtime/installations it does not.
The cross-family rule is simple: a declaration true for every host belongs here; a tenant-bound
installation belongs to Flux Exchange; execution semantics belong to Flux.

## The connector declaration

**Connector** — a compiled declaration of what one vendor can do in both directions and what an
operator must supply. Provider TOML is compiler input; the emitted Flux module and connector
manifest/catalogue are the artifacts. A Connector is not a process, credential, tenant connection or
installed app.

The published Rust catalogue still names this value `connector_catalog::Provider` and its lookup
`ProviderKey`. Those are compatibility API names. New family prose says **Connector**; “provider” is
qualified as **Model Provider** or **Identity Provider** when it means something else.

**Service** — one API surface within a Connector. It owns an endpoint/version and partitions the
Connector's operations. There is no standalone `Service` value in the published Rust catalogue:
service identity travels as a `service` field on `Operation`, `Event`, `Channel`, `ConfigField` and
`ConfigChoices`.

**Operation** — one outbound callable vendor action, compiled to Flux and projected by
`connector-pack` onto Flux's universal operation/tool contract. It declares schemas, effects, risk,
idempotency, direction, required credentials, hosts and service. It contains credential references,
never values.

**Event Type** — one inbound event name/schema the vendor can emit. It is a declaration, not an
occurrence and not a delivery queue. The published catalogue carries it as `Event`.

**Channel Binding** — a declarative composition of a transport, selected Event Types, verification
or cursor requirements, and an optional ordinary reply Operation. It declares; it never installs,
opens a socket or accepts a webhook. The published catalogue carries it as `Channel`.

**Graph** — a connector-owned composition that lowers to one Flux Operation. It is not a second
workflow runtime. No Connector declares one today, so the lowering
(`crates/connector-flux/src/graph.rs`) is not a live catalogue capability.

## What a host adds

- A **Connection** is a tenant installation of a Connector, including stable instance identity,
  settings and credential addresses. Flux Exchange owns it.
- A **Channel** is a deployment-scoped installed runtime surface created from a Channel Binding.
  Flux or Flux Exchange hosts it; `connector-pack` publishes no channel runner.
- An **Event Delivery** is one occurrence and its delivery outcome. A Connector declares its Event
  Type but never retains or retries a delivery.
- A **Trigger** is an installed binding from an event source to a Flux Program target. It is not a
  Connector member and does not belong in provider TOML.
- A **Datasource** is a governed readable record surface. Flux publishes the datasource contract;
  this repository's published closure carries no vendor-data Datasource Definition or live backend
  — `[[datasources]]` is C-512's IR member and C-513's artifact, both open. The proposed catalogue
  datasource is discovery *about connectors*, not live vendor data.
- An **App** is an installed Flux Program and a **Managed Agent** is a Flux Agent hosted inside it.
  Neither is a Connector.

## One namespace, two directions

Operations, Event Types, Channel Bindings, configuration fields and Graphs share one member
namespace per Service. A Channel Binding may name only declared Event Types and reply through a
declared Operation. This is why inbound and outbound are one Connector rather than two unrelated
integration models.

Secrets remain references the host resolves. No term on this page grants authority by existing:
installation and grants are explicit host actions.
