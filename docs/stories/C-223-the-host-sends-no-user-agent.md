---
id: C-223
title: "Every outgoing request leaves the host with no `User-Agent`, and at least one shipped vendor answers that with a 403"
pillar: Bridge
status: ready
priority: 2
design:
epic: connectors-api
areas: [bridge, host, connector-pack]
note: "found by the C-222 implementor 2026-07-31 and verified by the coordinator against the locked flux-web 0.41.0: two `Client::builder()` sites in egress.rs, neither calls `.user_agent()`, and reqwest has no default. Resend refuses such a request with a valid key"
---

# Every outgoing request leaves the host with no `User-Agent`

## Goal

Make the host identify itself on the wire, so that a vendor which requires a `User-Agent` gets one
and the operator is not debugging a 403 that names the wrong cause.

## What was measured

`codewandler-flux-web` **0.41.0** — the version this workspace's `Cargo.lock` actually resolves —
builds its HTTP client at two places in `src/egress.rs`:

```
egress.rs:22    Client::builder()
egress.rs:153   Client::builder()
```

Neither calls `ClientBuilder::user_agent`. `WebOptions` carries no field for one. `reqwest` sends no
default `User-Agent` of its own. So every request that leaves `connectors-api` — which binds
`HttpRequestTool` at `crates/connectors-api/src/state.rs:108` — goes out with no `User-Agent` header
at all.

**This was verified, not inferred.** The C-222 implementor found it while re-deriving a claim in the
preserved Resend work, and the coordinator confirmed it independently against the locked crate
source before filing.

## Why it is not cosmetic

Resend **rejects** a request with no `User-Agent`, with a `403` — carrying a perfectly valid API
key. That is the worst shape a failure can take for an operator:

- the status code says *authorization*, and the cause is a *missing header*;
- the credential is the obvious suspect, so the operator rotates a key that was never wrong;
- rotation changes nothing, and there is no signal anywhere pointing at the real cause.

Resend is simply the first shipped connector to make it visible. A vendor that requires a
`User-Agent` is a normal vendor; GitHub has documented the same requirement for years, and
[C-52](C-52-provider-github.md) already declares one as a constant request header per
[C-55](C-55-constant-request-headers.md). The difference is that a *per-connector* constant header
is a workaround each connector pays for separately — the host still has no identity of its own.

## The entry this closes, and why the previous reasoning went stale

`AGENTS.md`'s "Intentional gaps" carried an entry saying no HTTP implementation is bound here, so
whether a host supplies a `User-Agent` **cannot be checked**. That entry is marked **CLOSED
2026-07-31**: `codewandler-flux-web` is in the lock and `connectors-api` binds it. The question moved
from unanswerable to answerable, and the answer is *no*.

## Acceptance

- [ ] **Failing-first test:** an operation rehearsed or executed through the host's real transport
      carries a `User-Agent`. It fails today. Name it.
- [ ] Decide **where** the identity belongs and record the reason. The three candidates are not
      equivalent: the host setting it once on the client it builds; `connector-pack` setting it as
      part of request assembly; or each connector declaring it as a C-55 constant header. The last
      is what Resend and GitHub are each doing separately and is the one to argue *against*
      explicitly rather than to inherit by default.
- [ ] The value names this software and its version rather than a browser or a bare product word.
      A `User-Agent` that lies is worse than one that is absent.
- [ ] A connector that declares its **own** `User-Agent` as a constant header still wins over the
      default. Check what the two do when both are present — a duplicated header is its own defect,
      and this is the case that will occur first, because GitHub already declares one.
- [ ] The `DryRunTransport` from [C-145](C-145-dry-run-transport.md) reports the same header the live
      path sends. A rehearsal that differs from the wire is the thing that story exists to prevent.
- [ ] If the fix belongs upstream in `flux-web` rather than here, say so plainly and record what this
      repository does in the meantime — a pin, a wrapper, or a documented limitation. Do not leave
      the workaround undescribed.

## Notes

- **The blast radius is every connector, not one.** Vendors that require a `User-Agent` are a
  minority, but the ones that do fail closed and fail confusingly. This is cheap to fix once and
  expensive to diagnose 45 times.
- Related but distinct: [C-55](C-55-constant-request-headers.md) is about a *connector* declaring a
  constant header, which already works. This story is about the *host* having an identity at all.
- Do not fix this by adding a `User-Agent` to `providers/resend.toml` alone. That makes the symptom
  go away for one connector and leaves the mechanism exactly as it is — which is the shape
  [C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md) is already an instance of, one
  validation rule spelled in two places.
