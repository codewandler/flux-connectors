# flux-connectors — vision & principles

This document states *why* flux-connectors exists and the principles that decide how it's built. It
is the **tie-breaker** when a design choice is unclear: prefer the option that best serves the north
star and the principles below.

## What flux-connectors is

flux-connectors compiles **vendor API descriptions into Flux-Lang**. A provider is described once in
`providers/<name>.toml` — usually little more than a pointer at the vendor's OpenAPI document plus a
handful of patches — and the build emits a `<name>.flux` module of typed `op` declarations plus a
`<name>.connector.toml` manifest carrying what an `op` cannot say. [flux](../../flux) loads the module
from `~/.flux/flows` and every `op` becomes a first-class operation, exposed to the model as an LLM
tool; a host reads the manifest for everything around them.

A connector describes **both call directions**. Outbound is the operations flux invokes. Inbound is the
events the vendor sends *us* — a ticket updated, a call ended, a payment settled — declared in the same
provider TOML, with the signature scheme that authenticates them compiled rather than hand-written, and
the subscription that registers them emitted as an ordinary op. An integration that only knows how to
make calls is an API client; the reverse direction is the half real automations are built on. See
[designs/inbound-events.md](designs/inbound-events.md). Note what this does *not* change: inbound is
still **compiled, not hosted** — no endpoint, no relay, no daemon (see the non-goals below).

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

4. **Secrets are references the host resolves.** A credential never appears in a provider TOML, in a
   generated `.flux` file, or in a lockfile. The generated call carries an auth *reference*; flux
   resolves it, applies the scheme, and registers the value with the redactor.

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

- **Technology adapters.** Connectors are **paid SaaS services**. The flux plugins that wrap
  *technologies* — docker, kubernetes, sql, prometheus, loki, vault, asterisk — are stateful and
  protocol-rich, and they stay core to flux as plugins. Real Rust earns its keep there.

  What spans both repositories is the **contract**, not the code. A capability contract — "resolve a
  secret", "store a credential" — can be satisfied by a hand-written flux plugin (Vault) *and* by a
  generated connector (1Password), and a host that depends on the contract should not have to know
  which one it holds. That is a shared vocabulary, and it moves nothing across the line: Vault stays a
  flux plugin, and no technology adapter becomes generatable here because a contract happens to name
  it. See [designs/connector-contracts.md](designs/connector-contracts.md).
- **A runtime for production traffic.** flux executes connectors in anger. This repo may ship a
  **reference host** — `crates/connectors-app` — that proves the seams end to end: the OAuth callback,
  the credential store, an operation actually executing against a vendor. It is loopback-bound, never
  published, and never a production request path. Its only job is to make the seams demonstrable
  instead of asserted.

  That narrowing is how **C-34** resolves: **yes, narrowed** — yes to a host that proves the seams, no
  to the credential-injecting proxy the story was filed about, which is still a server, a daemon and a
  request path. See [designs/connectors-app.md](designs/connectors-app.md).
- **Universal API coverage.** A connector selects the members worth exposing — the operations, and the
  events with them. Mechanically emitting all 400 endpoints of a large spec produces an unusable tool
  catalog, not a good integration.
- **Replacing flux's native model providers.** flux talks to Anthropic and friends through
  `flux-providers`. A generated LLM-vendor connector is a pipeline test fixture and a convenience
  surface, not the inference path.
