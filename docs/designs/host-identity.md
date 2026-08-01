# Host identity on the wire

**Status: accepted, implemented by [C-223](../stories/C-223-the-host-sends-no-user-agent.md).**

Every request leaving this repository names the software that sent it. This record is about *where*
that name is attached and why the two other candidate positions lose, because the question is not
cosmetic and the three answers are not equivalent.

## The measurement that opened it

`codewandler-flux-web` **0.41.0** — the version this workspace's `Cargo.lock` resolves — builds its
HTTP client at two places in `src/egress.rs` (lines 22 and 153). **Neither calls
`ClientBuilder::user_agent`.** `WebOptions` carries no field for one. `reqwest` sends no default of
its own. So every request through `connectors-api`, which binds `HttpRequestTool` as the `Egress`,
went out with no `User-Agent` at all.

Reproduced on the wire rather than read out of a crate: before C-223,
`connectors-api/tests/live_egress.rs`'s loopback vendor recorded

```
["accept", "authorization", "content-length", "content-type", "host"]
```

for a shipped operation carried by the real transport.

### Why it is not cosmetic

Resend rejects a request with no `User-Agent` with a **`403`**, carrying a perfectly valid API key.
That is the worst shape a failure can take: the status says *authorization*, the cause is a *missing
header*, the credential is the obvious suspect, and rotating it changes nothing. GitHub has
documented the same requirement for years. Vendors that require a `User-Agent` are a minority, and
the ones that do fail closed and fail confusingly.

## The decision

**The identity is attached during request assembly, in `connector-pack`** —
`request::build`, via `identify`, after the URL guard and before the request is returned.

That function is the single funnel every path already shares:

| path | reaches `request::build` via |
|---|---|
| the live call | `Operation::build_authenticated_request` → `Operation::build_request` |
| the rehearsal (C-145) | `DryRunTransport::dry_run` → `Operation::build_request` |
| the boundary test (C-233) | `Rehearsal::request` |

Agreement between the rehearsal and the wire is therefore **structural** — one insertion point, not
two code paths maintained in parallel.

### The value

```
flux-connectors/<workspace version> (+https://github.com/codewandler/flux-connectors)
```

A product token and its version per RFC 9110 §10.1.5, with the repository as the comment a vendor can
act on. Both halves are read from the manifest (`CARGO_PKG_VERSION`, `CARGO_PKG_REPOSITORY`) rather
than typed, so neither goes stale at a release. It names *this* software rather than a browser or a
bare product word: **a `User-Agent` that lies is worse than one that is absent**, because a vendor's
rate limit, allow-list and support desk all believe it.

The test that asserts this asserts the **first product token**, whole and equal — not that the value
*contains* `flux-connectors`. The weaker form was written first and a mutation proved it worthless:
`Mozilla/5.0 0.7.0 (+…/flux-connectors)` satisfied it, because the repository URL in the comment
contains the product name. A test whose entire purpose is refusing a `User-Agent` that lies accepted
the canonical lie.

## Why not the host

`connectors-api` was the intuitive home and it loses on two independent grounds, either sufficient.

1. **It constructs no request.** `AGENTS.md`'s ownership table forbids it — *"Must never: construct a
   request of its own; every route ends in `connector_pack::pack`"* — since C-413, `pack` **or** `resolve`, both in `connector-pack`.* Its only lever is the client it
   builds, and flux-web 0.41.0 exposes no `user_agent` setting to build it with. A host-side fix is
   therefore either an upstream change or the host reaching into an assembled `Request`, which is the
   ownership boundary itself.
2. **A client-level header is invisible to the dry run.** `DryRunTransport` is a unit struct and its
   zero size is its whole safety argument — it holds no client, so it cannot report one's defaults.
   The rehearsal would describe a request the host does not make, which is precisely what C-145
   exists to prevent. **This ground stands even if flux-web grows the field**, which is why it is the
   decisive one.

Ground 2 is executed, not asserted: moving the insertion into `build_authenticated_request` — the
shape a host or transport fix would have — turns
`the_vendor_receives_a_user_agent_that_names_this_software` red on the dry-run comparison alone.

## Why not a per-connector constant header (C-55)

This is what `providers/resend.toml` does today and what [C-52](../stories/C-52-provider-github.md)
contemplated for GitHub. It is the option to **argue against explicitly** rather than inherit,
because it already exists and looks like a precedent.

- **The default becomes absence.** Forty-five connectors each pay for it separately, and the
  forty-sixth's omission is silent — surfacing as a vendor `403` that names authorization. Nothing
  fails at build time, because nothing can: a connector not declaring a header is a connector, not an
  error.
- **A TOML literal cannot carry the build's version.** Resend's shipped value is `"flux-connectors"`
  — a bare product word with no version, which is exactly what the acceptance rules out. Any
  per-connector value is wrong at the next release and nothing will notice.
- **It puts the host's identity in compiler data.** The compiler has no business having an opinion
  about which software executes its output; `connectors-api` is one host and this repository does not
  assume it is the only one.

## What happens when a connector declares its own

**It wins, and there is never a second header.** `identify` yields to any existing header whose name
matches `User-Agent` **case-insensitively**.

The case-insensitivity is the defect-prevention half, not fastidiousness. `Request::headers` is a
`BTreeMap`, so a module setting `user-agent` beside a default inserting `User-Agent` is two entries,
two JSON keys in `to_params`, and — depending on how the transport folds them — two headers on the
wire or a silent overwrite. A duplicated `User-Agent` is its own defect.

No shipped connector spells it in another case, so that half is proved against a doctored copy of
Resend's own declaration rather than against the catalogue. With the guard made case-sensitive, the
fixture reports the real thing: `["User-Agent", "content-type", "user-agent"]`.

## Where this belongs upstream, and what this repository does meanwhile

**The right long-term home for a client default is `codewandler-flux-web`**: `ClientBuilder::user_agent`
at both `egress.rs` builder sites, or a `WebOptions` field, so that *any* host and any tool reaching
that client is identified — including callers that never touch `connector-pack`. This repository
cannot make that change: the pin is a crates.io version and must stay one.

**This is not a workaround awaiting removal.** Even after upstream lands it, the pack-side identity
stays, for the dry-run reason above — the rehearsal has no client to read a default from. The two
compose rather than conflict: reqwest treats a per-request header as an **override** of a client
default, so a future flux-web default would apply to everything else and be superseded here, with no
duplication.

The interim state, stated plainly so it is not left undescribed: **requests assembled by
`connector-pack` are identified; anything else a host sends through flux-web directly is not.** In
this repository nothing else does — `connectors-api` sends only what the pack builds, and
`dependency_fence.rs` keeps the compiler off the network entirely.

## Consequences

- `Request::to_params` now always carries a `headers` record. The branch omitting an empty one still
  exists and is still correct; nothing in the shipped catalogue can reach it.
- `user-agent` came off `live_egress.rs`'s `TRANSPORT_HEADERS` exclusion list. It was there on a true
  premise — nothing in this repository set one — and the premise was the defect. It is now compared
  like any other pack-authored header, which is what proves the identity survives the wire
  byte-identically rather than merely being built.
