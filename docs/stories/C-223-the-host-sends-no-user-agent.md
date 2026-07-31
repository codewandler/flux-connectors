---
id: C-223
title: "Every outgoing request leaves the host with no `User-Agent`, and at least one shipped vendor answers that with a 403"
pillar: Bridge
status: in-progress
priority: 2
design: docs/designs/host-identity.md
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

- [x] **Failing-first test:** an operation rehearsed or executed through the host's real transport
      carries a `User-Agent`. It fails today. Name it.
      → `connectors-api/tests/live_egress.rs::the_vendor_receives_a_user_agent_that_names_this_software`.
      At the merge base the loopback vendor recorded
      `["accept", "authorization", "content-length", "content-type", "host"]` and no `user-agent`.
- [x] Decide **where** the identity belongs and record the reason. The three candidates are not
      equivalent: the host setting it once on the client it builds; `connector-pack` setting it as
      part of request assembly; or each connector declaring it as a C-55 constant header. The last
      is what Resend and GitHub are each doing separately and is the one to argue *against*
      explicitly rather than to inherit by default.
      → **`connector-pack` request assembly**, `request::identify` called from `request::build`.
      Both losers argued explicitly in [docs/designs/host-identity.md](../designs/host-identity.md)
      and at the function. One correction to the story's premise: **GitHub declares no `User-Agent`
      today** — `providers/github.toml` declares only `Accept`. Resend is the sole instance.
- [x] The value names this software and its version rather than a browser or a bare product word.
      A `User-Agent` that lies is worse than one that is absent.
      → `flux-connectors/0.7.0 (+https://github.com/codewandler/flux-connectors)`, both halves read
      from the manifest. The test asserts the **first product token**, whole and equal; the weaker
      `contains("flux-connectors")` form was written first and a mutation showed it accepted
      `Mozilla/5.0 …` because the repository URL in the comment satisfied it.
- [x] A connector that declares its **own** `User-Agent` as a constant header still wins over the
      default. Check what the two do when both are present — a duplicated header is its own defect,
      and this is the case that will occur first, because GitHub already declares one.
      → `connector-pack/tests/request.rs::a_connector_declaring_its_own_user_agent_wins_and_gains_no_second_one`.
      Resend keeps its own value; the guard is **case-insensitive**, because `Request::headers` is a
      `BTreeMap` and `user-agent` beside `User-Agent` is two entries. Made case-sensitive, the
      fixture reports `["User-Agent", "content-type", "user-agent"]`.
- [x] The `DryRunTransport` from [C-145](C-145-dry-run-transport.md) reports the same header the live
      path sends. A rehearsal that differs from the wire is the thing that story exists to prevent.
      → Structural, not parallel: both reach `request::build` through `Operation::build_request`. The
      live test compares the rehearsal against what the **vendor actually received**. Moving the
      insertion to `build_authenticated_request` — the shape a host-side fix would have — turns that
      comparison red.
- [x] If the fix belongs upstream in `flux-web` rather than here, say so plainly and record what this
      repository does in the meantime — a pin, a wrapper, or a documented limitation. Do not leave
      the workaround undescribed.
      → A client default belongs upstream (`ClientBuilder::user_agent` at both `egress.rs` builder
      sites) and this repository cannot make it — the pin is a crates.io version. **But the pack-side
      identity is not an interim workaround**: the dry run has no client to read a default from, so
      it stays either way, and a per-request header overrides a client default rather than
      duplicating it. Interim limitation recorded in the design: requests the pack assembles are
      identified, anything a host sends through flux-web directly is not — and in this repository
      nothing else does.

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

## Progress

**Landed on `impl/C-223`. Gate fully green: 1282 passed, 0 failed across 136 binaries; clippy and
`fmt --check` clean.** This story adds no provider, so none of the eight whole-catalogue staleness
failures apply and none appeared.

The change is small and its position is the whole point. `request::identify` runs at the end of
`request::build` — the one funnel `Operation::build_request`, `build_authenticated_request`,
`DryRunTransport::dry_run` and `Rehearsal::request` all already pass through — so rehearsal/wire
agreement is a property of the code path rather than of two places kept in step.

**Every test here was falsified by mutation**, which changed the work twice:

| mutation | red | green (correctly) |
|---|---|---|
| remove the `identify` call | the wire test, the catalogue property, 3 exact-header tests | the Resend test — it declares its own |
| overwrite instead of yielding | the Resend test, alone | everything else |
| make the guard case-sensitive | the lowercase fixture, alone | the catalogue property — no connector spells it that way, which is why the fixture exists |
| move `identify` to `build_authenticated_request` | the dry-run comparison in the wire test | the built-vs-received equality |
| `Mozilla/5.0` as the product token | **nothing — the test survived it** | — |

The last one is the finding worth carrying forward. `contains("flux-connectors")` was satisfied by
the repository URL inside the value's own comment, so the assertion guarding against a lying
`User-Agent` accepted the canonical lie. It now asserts the first product token whole and equal.

Two things changed outside the story's own ground, both consequences rather than scope creep:

1. `user-agent` came off `live_egress.rs`'s `TRANSPORT_HEADERS` exclusion. It was excused there
   because nothing in this repository set one; now the pack authors it, so it is compared like any
   other pack header — which is what proves it survives the wire byte-identically.
2. `Request::to_params` now always carries a `headers` record. The empty-record branch still exists
   and is still correct; nothing in the shipped catalogue can reach it.

### For a follow-up story, not done here

`providers/resend.toml` declares `const_headers = { "User-Agent" = "flux-connectors" }` — a bare
product word, no version, and now redundant. Removing it would let Resend inherit the versioned host
identity. **Deliberately not done**: editing a provider file requires `build --provider resend` and
leaves three whole-catalogue artifacts stale, and this story's gate permits zero red tests. It wants
a wave that owns the regeneration.
