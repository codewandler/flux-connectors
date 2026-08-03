# flux-connectors — vision & principles

This document states *why* flux-connectors exists and the principles that decide how it's built. It
is the **tie-breaker** when a design choice is unclear: prefer the option that best serves the north
star and the principles below.

## What flux-connectors is

flux-connectors is the source and distribution boundary for **every official integration**. For an
HTTP API it compiles a vendor description into Flux-Lang. For a protocol-rich integration it will
also bind operations to an attested runtime artifact. A provider is described once in
`providers/<name>.toml` — usually little more than a pointer at the vendor's OpenAPI document plus a
handful of patches — and the build emits a `<name>.flux` module of typed `op` declarations plus a
`<name>.connector.toml` manifest carrying what an `op` cannot say. Exchange loads and executes that
compiled contract; [flux](../../flux) reaches it through its embedded Exchange client and never
receives the vendor credential or runtime artifact. A host reads the manifest for everything around
the operations.

A connector describes **both call directions**. Outbound is the operations flux invokes. Inbound is the
events the vendor sends *us* — a ticket updated, a call ended, a payment settled — declared in the same
provider TOML, with the signature scheme that authenticates them compiled rather than hand-written, and
the subscription that registers them emitted as an ordinary op. An integration that only knows how to
make calls is an API client; the reverse direction is the half real automations are built on. See
[designs/inbound-events.md](designs/inbound-events.md). Note what this does *not* change: inbound is
still **compiled, not hosted here** — no endpoint, no relay, no delivery daemon. Exchange owns the
channel termination and retained delivery; the reference host in this repository runs operations
only and does not change that boundary.

The defining idea is **one abstraction level up from a plugin**. Integrating Zendesk into flux today
means writing a stdio plugin — a large hand-written artifact for roughly seven operations.
(The specific `plugins/zendesk/src/main.rs` this originally cited turned out to be uncommitted
working-tree material in the flux checkout and is now gone; see
[designs/zendesk-plugin-citation.md](designs/zendesk-plugin-citation.md). The argument stands; the
number is no longer checkable.) Almost everything those lines encode — base URL, auth kind, endpoint,
parameters, response shape — the vendor already publishes as a spec. A connector is what remains once
you stop hand-writing the part a machine can derive.

What remains is wider than "auth plus a list of endpoints", and an earlier draft of this document said
otherwise for long enough that the docs downstream of it inherited the mistake. The framing that holds:
**a connector declares what a vendor can do in both directions, and what an operator must supply to use
it.** `Connector` (`../crates/connector-spec/src/ir.rs`) carries sixteen fields to say that —
`operations` and `services` outbound, `events` and `channels` inbound, `auth` and `default_auth` across
both, `config` for what a human types before any of it runs, `graphs` for a flow composed from the
members above, a service's `roles` for the capability shape it claims, and `verify` for the one cheap
read that proves the whole arrangement works. All of it shares **one name namespace per service**,
enforced by `Connector::member_names_of`, because a config field and an operation resolving to one name
would be ambiguous wherever a host looked either up. The full surface, field by field and with what
reaches which artifact, is [designs/connector-surfaces.md](designs/connector-surfaces.md).

## Vocabulary and ownership

- A **Connector** is the complete compiled vendor declaration. The public catalogue's Rust type is
  still named `Provider` for compatibility; prose says Connector. A **Model Provider** supplies
  inference and an **Identity Provider** authenticates a caller—neither is a Connector.
- A **Service** is one Connector API surface with its own endpoint/version and operation partition.
  It is metadata inside the Connector, not a separately installed resource.
- An **Operation** is one callable unit. A Tool is Flux's model-visible projection of an operation;
  this repository does not define another execution unit.
- An **Event Type** and **Channel Binding** are declarations of inbound schema and transport
  requirements. A host installs and terminates the Channel, retains Event Deliveries and binds
  Triggers. A webhook or WebSocket is transport, not a Trigger or Event synonym.
- A **Credential Requirement** and configuration field state what an operator must supply. The host
  owns Connections and their stable instance identities; connectors receive only the resolved UUID
  and render the shared address vocabulary.
- Datasource Definitions are a known upstream gap. Apps, Managed Agents, Service Accounts,
  Datasources, Triggers, Event Deliveries, tenants and grants are host/Flux concepts, not additional
  Connector resources to invent here.

The ownership test is simple: if it is true of the vendor regardless of who runs it, it belongs in
flux-connectors. If it requires a tenant, installed binding, held credential, retained delivery or
runtime execution, Exchange owns it. Flux owns the agent loop, model-facing projection and approval,
and reaches official integrations only through its embedded Exchange client.

## North star

**A connector is compiled, never interpreted.** The TOML is input to a compiler; the artifact that
runs is Flux — a real typed language with an analyzer, a formatter, and first-class `retry`,
`throttle`, `saga`, and approval gates. Any proposal that moves behavior back into config the runtime
reads directly is wrong, however convenient it looks.

## Principles

1. **The vendor spec is the source of truth; drift is detected, not absorbed.** Hand-maintained
   integration config silently diverges from the real API forever. Every generated artifact records
   the hash and version of what produced it, and `flux-connectors check` fails when upstream moves.

2. **No homegrown DSL.** Interpolation, branching, and error handling are expressed in Flux, which
   already has a parser, an analyzer, and editor tooling. We never invent a second little language
   to sit in front of it.

3. **Generated code is committed and reviewed.** Generation is an explicit CLI run producing a diff
   a human reads in a PR — not build-script magic and not a network call at runtime. Builds are
   hermetic, offline, and reproducible from a vendored spec cache.

4. **Secrets are references Exchange resolves.** A credential never appears in a provider TOML, in a
   generated `.flux` file, in a lockfile or in Flux. The generated call carries an auth *reference*;
   Exchange resolves it, applies the scheme, and registers the value with its redactor.

5. **A connector declares what it needs, and nothing grants itself access.** A connector is a
   manifest plus a Flux module, and the manifest names the hosts it reaches, the credentials it
   requires, and the configuration it asks an operator for. Note the asymmetry with plugins,
   established by C-16: flux obtains a plugin's capabilities by *spawning its binary*, and has no
   file-based capability manifest at all, so a connector manifest is a build artifact and a
   declaration — not a self-installing capability grant.
   Access is granted by operator configuration, deliberately.

6. **Types survive the whole pipeline.** Parameter and response schemas travel from the vendor spec
   through the IR into the op contract. An operation that takes an integer says so.

## Non-goals

- **Owning generic execution mechanisms.** This repository owns connector declarations and any
  vendor-specific runtime artifacts they require. It does not own a generic Kubernetes, database,
  socket, process, container, or plugin execution engine. Exchange composes generic mechanisms behind
  tenant authority. A framed stdio protocol may remain as one connector-owned runtime artifact kind,
  but it is not a Flux plugin or release artifact. See
  [the accepted migration design](designs/all-integrations-are-connectors.md).
- **Creating a second official execution placement.** Exchange is mandatory for official external
  integrations. Flux remains useful without it for the language, agent loop and core tools, but has
  no local connector, vendor-adapter or plugin fallback. This repo's `crates/connectors-api` binary
  remains a reference/development host for the existing HTTP seam; it is not a parallel supported
  product boundary.

  **Historical context:** this non-goal was amended on 2026-07-31, by the owner. It previously read
  *"A runtime for production traffic"* and narrowed the host to a `crates/connectors-app` that was *"loopback-bound,
  never published, and never a production request path"* — the **yes-narrowed** resolution of
  [C-34](stories/C-34-proxy-charter-decision.md). The owner directed the wider shape: a
  deployed, multi-tenant service an operator signs into, connects providers to, and calls operations
  from. [C-200](stories/C-200-connectors-api-epic.md) is the epic;
  [C-201](stories/C-201-charter-multi-tenant-host.md) is this amendment.

  C-507 and flux-roadmap Decision 0001 supersede that production-host destination while retaining the
  harness and its safety evidence. [designs/connectors-app.md](designs/connectors-app.md)
  keeps the reasoning it rested on — the `Egress` analysis, the slice-1 sequence, and why a host that
  builds its own requests is the failure mode — and
  [designs/connectors-api.md](designs/connectors-api.md) records what replaces it and answers
  [designs/connectors-proxy.md](designs/connectors-proxy.md)'s confused-deputy objection, which the
  amendment does **not** get to skip. C-34's "no" to the credential-injecting proxy still stands: the
  thing rejected was a service that adds authority to *whoever asks*, and no amount of deployment
  makes that acceptable.

  Four things the amendment does **not** license, none of which follow from it:

  - **A second request path.** The host constructs no request of its own. This is the structural
    reason `connectors-app` superseded `connectors-proxy`, it is unaffected by tenancy, and it
    survives verbatim.
  - **Publication.** `publish = false` on the host crate. The amendment is about *deployment*, not
    crates.io; the publish closure stays four crates
    ([C-190](stories/C-190-publish-catalog-pack-secrets.md)).
  - **A reachable bind before an authenticated principal exists.** The host is loopback-only today
    and stays that way until the session is what names the tenant. Widening the bind first is
    precisely the rejected proxy.
  - **Holding credentials the operator did not choose to give it.** A tenant's credentials are
    reachable only by that tenant's own authenticated session.
- **Universal API coverage.** A connector selects the members worth exposing — the operations, and the
  events with them. Mechanically emitting all 400 endpoints of a large spec produces an unusable tool
  catalog, not a good integration.
- **Replacing flux's native model providers.** flux talks to Anthropic and friends through
  `flux-providers`. A generated LLM-vendor connector is a pipeline test fixture and a convenience
  surface, not the inference path.
