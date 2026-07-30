# Design: credential addressing, and the secret-store seam

**Status:** accepted (the pure layer landed) · **Pillar:** Spec (+ Bridge) ·
**Epic:** `credential-addressing` · **Stories:** C-90 … C-93

## Why

The [configuration surface](connector-configuration.md) modelled *what a human supplies*. It stopped
at the boundary: once a value is collected, **where is it kept?** Both that design and this
repository's notes recorded the answer as out of scope — flux's store is single-tenant.

A hosted product cannot leave it there. Two customers connecting the same Zendesk need two different
tokens, and nothing in either repository can say where the second one goes.

## The deliverable is a convention, not a client

`docs/vision.md`'s non-goal is load-bearing: *"A runtime. This repo compiles; flux executes. This repo
ships no server, no daemon, and no request path of its own."* A store trait with a Vault client is
runtime code, and putting one in the compile path would change what this repository is.

So the split is:

| | owner | why |
|---|---|---|
| **the address** | flux-connectors, `connector-spec` | pure, no IO. This repo already owns `pid`/`gid`/`oip`, validates every component, and refuses an address it cannot spell. A credential path is one more address derived from the same facts. |
| **the store** | a host library (C-90), outside the compile path | opens sockets. `connector-cli` must not depend on it, so `tests/no_network.rs` keeps meaning what it means. |

`Layout` is the seam between them. *"Wrap a simple Vault store with some conventions"* is exactly a
decorator: the client is commodity, the convention is the part worth owning.

## What the remembered path actually is

`tenants/{uuid}/cloud/google/gemini` does not exist. It is a conflation of three real conventions:

| source | convention | notes |
|---|---|---|
| action-proxy | `customer/<accountUuid>/integrations/<integrationUuid>` | Vault KV **v1**; both ids arrive as **unvalidated client headers** |
| `credentials-store` (Go) | `cloud/<provider>/<service>` | **no tenant segment at all**; no `gemini` key exists |
| `sbf/secrets` | `tenants/<tenantID>/{static/<prefix>\|credentials/<credentialID>}` | KV v2, tenant derived server-side from an introspection claim |

`sbf/secrets` is the real precedent for `tenants/`, and its own justification is that action-proxy's
approach was fragmented across four stores with split-brain between Vault and mongo. Worth knowing
before adopting either wholesale.

Note also what action-proxy's shape *cannot* say: `integrations/<uuid>` is an opaque row id, so
nothing about the path tells you which API it opens. Putting the authority in the path is the one
substantive difference here.

## Shape

```
tenants/<tenant>/<authority>/<service>/<credential>

tenants/9f3a…/com.slack.api/signing_secret          ← `default` service elided
tenants/9f3a…/com.zendesk.api/support/api_token
tenants/9f3a…/com.amazonaws/s3/access_key
```

The tenant leads because it is the segment a store's access control is written against: a Vault policy
scoping a token to one customer is a prefix rule, and a prefix rule wants the tenant first.

### The API version is deliberately absent

A `gid` is `authority/service:version`. A credential path uses the **`pid` plus the service** and
drops the version, because **a token must survive the vendor's v2 migration**. Putting the version in
the path would force every tenant to re-provision the day Zendesk ships a new API version — backwards,
since the credential is precisely the thing that did *not* change.

A useful consequence: a path needs only an `authority`, not an `api_version`, so a provider is half as
far from having one.

### The leaf drops the vendor prefix

`AuthMethod::name` is `zendesk.api_token` — vendor-prefixed because credentials share **one flat
namespace** across the connector. The path already carries the authority, so the leaf is `api_token`.
Carrying the prefix would say the same thing twice and put a `.` inside what must be one segment.
`Connector::local_credential_name` derives it and refuses a prefix that disagrees with the connector
id, which would otherwise render a plausible path under the wrong vendor.

### The service segment is headroom, not decoration

Credentials are declared at **provider** level, so every shipped path elides the service today. The
path can still carry one, deliberately: a vendor whose surfaces authenticate separately becomes
expressible the day one appears, without moving every path that already exists.

## Invariants

1. **A tenant id is untrusted input.** `CredentialRef::new` returns a `Result`; there is no way to
   construct one that renders a traversing path. Refused: empty, `.`, `..` anywhere, a leading or
   trailing `.`, any `/`, whitespace, control characters, and anything over `MAX_TENANT`.
2. **Every other component is re-validated**, through the existing `address::` validators, even though
   a loaded connector already checked them — a reference can be built from outside one, and a host
   resolving a path it was handed is exactly the case that matters.
3. **`parse(render(r)) == r`**, through the elision. Tested as a property over a deliberately hostile
   corpus with the validator as the gate, because the failure that matters is not an error — it is a
   segment that renders into a path to somebody else's secret.
4. **`default` never reaches a path**, and spelling it out explicitly does not parse. Two paths for one
   address is how a store ends up holding the same credential twice with nothing to say which is
   current. `Gid::parse` refuses it for the same reason.
5. **No value appears in this type.** `CredentialRef` is an address; it has no secret field, so it can
   be logged, compared and stored freely.

## What this cannot do, and must not pretend to

**Validating a tenant id does not make an attacker-supplied one safe to act on.** This crate refuses a
traversing id; it cannot vouch for *provenance*.

The two precedents differ exactly here, and it is the difference that mattered: `sbf/secrets` derives
the tenant server-side from an authenticated principal and documents that handlers *"must take the
tenant from here, never from input"*; action-proxy takes it from a client header. Deriving the tenant
is the host's job and stays the host's job — this design would be actively harmful if it left anyone
believing otherwise.

## What this does not settle

- **The store.** C-90: a `SecretStore` with `get`/`put`/`delete` and typed errors, plus a `VaultStore`
  behind a feature. Deliberately not this wave — a client is only worth writing against a live server
  or a mock.
- **Authorities.** Fifteen of sixteen providers declare none, so no path renders for them. C-91, and
  its own story because `global-addressing.md`'s risk register says *"choosing an authority commits
  us"* — minting fifteen published identifiers inside a wave about secrets would bury a real decision.
- **The flux adapter.** C-92, with a trap worth stating up front: flux's CLI write path
  (`save_token`/`delete_token`) is hard-wired to the file backend, so an injected store is **read-only
  in practice** until flux changes.
- **Expiring tokens.** Out of scope by instruction, and flux already has substantial machinery
  (`REFRESH_BUFFER_MS`, `FORCE_REFRESH_DEDUP_MS`, refresh-token rotation, a 60s Vault renew buffer).
  Do not re-specify it differently.

## Alternatives considered

- **Implement `flux_credentials::CredentialStore` directly.** Two methods, drops straight into
  `SystemHostCaps::with_credential_store`. Rejected as the *primary* shape because it has no `delete`
  (so `flux auth set --clear` cannot clear a Vault-backed credential), its `load` returns `Option` so a
  backend outage is indistinguishable from "not configured", and its key has no tenant. It remains the
  right *adapter* target — C-92.
- **A category taxonomy** (`cloud/google/gemini`). Reads well to a human browsing Vault, but the
  category vocabulary is a second naming scheme this repository would have to invent, own and keep in
  step with the addresses it already publishes.
- **Reuse the `gid`, version included.** Ties a credential's lifetime to the vendor's API version —
  see above.
- **Put the whole store here and amend the non-goal.** Honest, and a bigger change than the problem
  needs: the layout is the part nobody else can own, and it is pure.
- **Do it all in flux instead.** D-83's dropped `[+account]` should indeed be reopened there — filed as
  a handoff — but flux has no connector addresses, so it cannot derive this path.
