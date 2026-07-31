# Design: `connectors-api` — the multi-tenant host

**Status:** charter accepted, 2026-07-31, owner-directed · **Pillar:** Bridge · **Epic:**
[C-200](../stories/C-200-connectors-api-epic.md) · **Supersedes:**
[connectors-app.md](connectors-app.md)'s narrowing (not the document) · **Reopens and answers:**
[connectors-proxy.md](connectors-proxy.md)'s confused-deputy objection

> **Scope of this document.** It is the *charter* half, written by
> [C-201](../stories/C-201-charter-multi-tenant-host.md): what the amendment permits, what bounds it,
> and why a multi-tenant credential-holding host is not the thing C-34 rejected. The engineering —
> the tenancy model, the routes, the sign-in and the connect flows — belongs to the child stories in
> [C-200](../stories/C-200-connectors-api-epic.md)'s table and is not duplicated here. Where this
> document and the code disagree, the code is the finding.
>
> Note when following that table: its rows for C-205 through C-208 name stories that **do not exist
> under those ids**. `C-205` and `C-206` are filed as unrelated stories, and `C-207`/`C-208` are
> unfiled. Only C-201 through C-204 resolve. That is C-200's to reconcile, not this document's.
>
> `path:line` citations were read on **2026-07-31** at commit `1390f09`. Re-grep by symbol.

## What changed

`docs/vision.md` narrowed this repository's host to *"loopback-bound, never published, never a
production request path"*, and [connectors-app.md](connectors-app.md) narrowed it further to *"the
operator sitting in front of it"* holding *"one operator's own [credentials], in one process they
started"*. That narrowing was how [C-34](../stories/C-34-proxy-charter-decision.md) resolved as
**yes-narrowed**.

The owner directed a wider shape on 2026-07-31: a deployed service an operator signs into with
Google, connects providers to, and calls operations from. `crates/connectors-api` is that host, and
C-201 amended `vision.md` rather than leaving the crate in contradiction with it.

**What the amendment is not.** It is not a finding that the narrowing was wrong. The narrowing was
load-bearing — it was standing in for an argument that a multi-tenant host had not yet had to make.
Widening the charter without re-making that argument would delete the analysis instead of answering
it. §"The confused deputy, answered again" is the argument.

## What the host actually is today, measured

The gap between the charter and the code is the thing most likely to be misread, in the optimistic
direction, so it is stated first.

| | charter permits | `crates/connectors-api` today |
|---|---|---|
| bind | a reachable address, once the gate below is met | `Ipv4Addr::LOCALHOST`, port `8787`, **no flag, no env var** (`src/main.rs`) |
| principal | an authenticated account | **none** — `tenant_of()` returns the constant `SOLE_TENANT = "local"` (`src/api.rs:24-36`) |
| tenancy | many | the tenant is a **parameter of every port**, with one value bound |
| credential store | per tenant, persistent | per tenant, **a `0600` file by default** (C-207) — selected by `CONNECTORS_CREDENTIAL_STORE`, refusing a bad value rather than falling back to memory |
| transport | flux's | `flux_web::http::HttpRequestTool`, `PrivateNetAllow::None` |
| request construction | none of its own | none of its own — every route ends in `connector_pack::pack` |

So the host is, at the moment of writing, still loopback-only and single-valued. **What the charter
amendment buys is not a deployment; it is permission to build toward one**, plus the obligation to
say what has to be true first. Three of the six rows above are already the deployed shape — the
tenant threaded through every port, the transport, and the no-construction rule — and they were built
that way from the first commit precisely so the remaining three are additions rather than
retrofits.

## The confused deputy, answered again

[connectors-proxy.md](connectors-proxy.md) §"The proxy must be authenticated" is the objection:

> **A credential-injecting proxy is, by construction, a confused-deputy machine**: its entire job is
> to add authority a caller does not have.

`connectors-app.md` escaped this by removing the second principal: *"absent — the caller **is** the
principal."* A multi-tenant host cannot use that answer. It has many principals and holds many
tenants' credentials, which is the exact configuration the sentence above describes. The answer has
to be structural, and **"it is authenticated" is not it** — the proxy design already proposed
authentication as its mitigation and correctly judged it insufficient.

### The distinction that does the work: the caller cannot name the authority

The proxy's deputy problem comes from its interface, not from its deployment. Its caller submits a
request naming **a host** and **a credential**, and the proxy injects and forwards. Authority flows
from the service's store to whoever asked, in whatever direction they asked for. Authentication
bounds *who may ask*; it does nothing about the fact that asking is how the authority is selected.

`connectors-api` has no such interface. A caller names an **operation id** — nothing else about the
request is theirs to choose:

- **It cannot name a host.** The URL comes from the operation's own compiled Flux, evaluated by
  `connector_pack`. The host constructs no request (`src/lib.rs`), so there is no field in which a
  destination could be supplied. Egress is additionally bounded by flux's own allow-list and SSRF
  guard, with `PrivateNetAllow::None`.
- **It cannot name a credential.** The address is *derived*:
  `tenants/<tenant>/<authority>/<credential>`, where `<tenant>` comes from the session and
  `<authority>` and `<credential>` come from the operation's declaration in the catalogue. A provider
  with no declared `authority` yields `Error::NoCredentialAddress` and the request is refused
  (`crates/connector-pack/src/credentials.rs:126-149`) rather than sent unauthenticated.
- **It cannot name a tenant.** `tenant_of()` (`src/api.rs:26-36`) is the single seam, and its
  contract is that the tenant is read from the session and from *nothing a caller controls* — not a
  path segment, not a body field, not a header.

So the four questions the amendment owes:

**Who is the principal?** The signed-in account, and only that. Until sign-in lands
([C-204](../stories/C-204-google-signin-accounts.md)) there is no principal, and the loopback bind is
what stands in for one — which is why the bind may not widen first (§"The bind").

**What proves a caller may use a tenant's credential?** That the caller holds an authenticated
session *for that tenant*, and that the tenant that session names is the one both ports were
constructed with. The proof is not a claim in the request; it is that no other tenant value is
reachable from the request-handling path.

**What stops tenant A's session reaching tenant B's secret?** Address derivation plus validation, at
two levels. `Credentials::new(store, tenant)` validates the tenant as a usable path segment and
refuses *at construction* — empty, over-long, or *"a spelling that would traverse"*
(`crates/connector-pack/src/credentials.rs:101-113`, with `../../etc` asserted refused at `:434`) —
so a crafted tenant id cannot walk out of its own prefix. Every address the port then renders is
prefixed with that validated tenant. `Error::TenantMismatch` covers the remaining failure, a host
pairing two ports built for different tenants; the host builds both from one value at one call site,
which is what makes it unreachable rather than merely untriggered. Turning this from a construction
argument into an asserted test is the per-tenant-isolation story C-200's table calls `C-205`, which
is **not yet filed under that id** — it is the gate's item 2 below, and it is unowned.

**What does the service refuse to do?**

- **Forward.** It has no route that takes a destination. It runs catalogue operations.
- **Construct a request.** Every route ends in `connector_pack::pack`.
- **Echo a credential**, including on error.
- **Lend across tenants.** No route reads a tenant from caller-controlled input.
- **Bind a reachable address without a principal.** See below.

### Why this is custody rather than deputation

The compressed form, and the reason the amendment does not reopen C-34's "no":

> A deputy adds authority its caller **does not have**. This host returns authority its caller
> **deposited** — a tenant's own credential, to that tenant's own session, for an operation whose
> destination the tenant did not choose.

The rejected proxy could not make this argument, because its caller chose the destination and the
credential. That is the whole of the difference, and it is a property of the interface, so it holds
whether the process is bound to loopback or to the internet. **C-34's "no" still stands as written:**
a service that adds authority to whoever asks remains out of scope, and the first route that accepts
a caller-supplied host or credential address is the one to refuse.

## The bind

[connectors-app.md](connectors-app.md) §"Loopback is a property of the code, not of a flag" set the
rule that the host has *no configuration surface for its listen address at all*, on
[C-145](../stories/C-145-dry-run-transport.md)'s reasoning that *"a flag on a live client is
something a caller forgets"*. A host meant to be deployed cannot keep that rule permanently. The
reasoning under it survives, and converts it from a permanent prohibition into a **gate**:

**The bind address may widen beyond loopback only once all of the following hold. Not any of them —
all.**

1. `tenant_of()` reads a verified session and cannot return a value derived from request input
   ([C-204](../stories/C-204-google-signin-accounts.md)).
2. Per-tenant credential isolation is asserted by a test that fails if A's session reaches B's
   secret — not argued from construction.
3. The credential store at rest is a deliberate choice with its own design, not `MemoryStore`
   promoted by default. **Met for a single-operator deployment, and not for a reachable one**
   ([C-207](../stories/C-207-the-host-forgets-every-credential.md)): the store is now selected by
   `CONNECTORS_CREDENTIAL_STORE`, defaults to a `0600` file, and refuses a bad value rather than
   falling back to memory. What that file has is a mode; what it does not have is encryption, and
   the store says so in its own module docs, in the README and in the startup banner. A host bound
   to a reachable address holds several tenants' tokens recoverable by anyone who can read one
   file, so **widening the bind wants `VaultStore` or an equivalent, not this**. The gate item is
   satisfied in the sense that the choice is now expressible and defaulted deliberately; it is not
   a finding that a plaintext file is adequate for a deployment.
4. The widening is a reviewed change that cites this section.

Until then the constant stays a constant. **A PR that adds a `--bind` flag while `tenant_of()` still
returns a constant is the rejected proxy**, exactly and without qualification, and is the one to
refuse — the original prohibition's real target, restated so it survives the amendment.

## The dev sign-in (C-234)

C-204 made every `/v1` route require a session and made the tenant come from that session. The
consequence, which nobody costed at the time, is that **the only door into the app was a real Google
registration**: without a client id a developer could start the host, load the page, and reach not
one route. [C-234](../stories/C-234-a-dev-sign-in-that-needs-no-google-registration.md) adds a
second door, `POST /auth/dev`, behind a `--dev` flag on the binary.

It rests entirely on §"The bind" above. **The dev door is defensible only while the listen address is
loopback and unconfigurable**, so C-234 hardened that rather than leaning on it silently:
`main.rs::options()` now *refuses* unknown arguments instead of ignoring them, which turns
`--bind 0.0.0.0` from a word the binary shrugs at into a startup error naming the reason. Gate item 4
above applies to any future widening with redoubled force: **widening the bind while `--dev` exists
must revisit this section, not just that one.**

### Two judgement calls the story asked to be recorded

**1. `--dev` is *not* refused when a Google registration is also configured.** Both doors may stand
open on one process.

The argument for refusing is that one door is simpler to reason about. It does not survive contact
with what the two doors actually produce. `Account::from_claims` is the only constructor reachable
from an `id_token` and it prepends the literal `google-` to every tenant it makes;
`Account::developer` produces the literal `dev-local` and takes no arguments, so no request can steer
it. `Account`'s fields are private and those are its only two constructors, which makes the
namespaces disjoint *structurally* — not because Google's `sub` happens to be numeric, and not by a
collision probability. A credential pasted into a dev session is unreachable from a real one and vice
versa, and `tests/dev_signin.rs::a_dev_session_and_a_google_session_share_no_credential` drives both
doors on one process and asserts exactly that.

Against that, refusing costs real usability: it makes trying the real sign-in and the dev sign-in on
one machine a matter of unsetting environment variables, and an operator with a stale
`CONNECTORS_GOOGLE_CLIENT_ID` in a shell profile would get a startup failure instead of an app —
which is the first-run stack trace C-204 refuses by name, arriving through a different door. So both
are allowed, and the disjointness is what pays for it.

**2. A runtime flag rather than a cargo feature.** A `#[cfg(feature = "dev")]` would be strictly
stronger on one axis: the handler, the route and `Account::developer` would be physically absent
from a release build, so no runtime state could reach them. That is a real advantage and it is
declined for two reasons.

The first is the story's own point: `cargo run -p connectors-api -- --dev` has to work out of the
box, and a feature makes it `cargo run -p connectors-api --features dev -- --dev` — a longer
incantation for the exact person who cannot get in at all. The second is that the advantage is
mostly theoretical *here*: this crate is `publish = false` and ships no release binary, so "absent
from the release artefact" has no artefact to be absent from. The property the feature would buy is
already bought more cheaply by the route not existing in the table unless the flag was typed.

**This is worth revisiting the day this crate acquires a shipped artefact.** If a binary is ever
released or containerised, the flag should become a feature, because at that point an inherited
argv or an entrypoint wrapper becomes a plausible way to switch it on and "physically absent" starts
paying for itself. Recorded here so that change is a decision rather than a discovery.

### What it does not do

- **It does not touch C-204's login-CSRF binding.** The `connectors_login` cookie, its constant-time
  comparison against `state` and the single-use pending entry are unchanged. A security re-review
  reproduced the original cross-account credential-capture attack end to end and confirmed it dead on
  the fix; dev mode adds a door beside that one.
- **It does not add a second kind of session.** `POST /auth/dev` calls `Accounts::of_subject`,
  `Sessions::create` and `session_cookie` — the same three calls in the same order as the tail of
  `/auth/callback`. There is no flag on the session record and no branch downstream, so `Principal`,
  the tenant resolution, the TTL, the opacity and the server-side revocation are the same code under
  `--dev` as in production. A dev mode that special-cased any of those would make every other route
  behave differently under test than it does in production, which is what makes most dev modes cost
  more than they are worth.
- **It does not become a hole in the one invariant.**
  `tests/dev_signin.rs::a_credential_stored_in_dev_mode_reaches_no_surface` re-proves
  `tests/host.rs`'s sweep — no credential value on any served surface, including on error — through
  the dev door, over the new route as well as the old ones.

### The residual risk

The dev route mints a session from a request carrying no cookie, so `SameSite=Lax` — which is what
stops a third-party page driving the credential-writing routes — does nothing for it, and a
cross-site form `POST` would otherwise put a browser into the dev tenant silently. That is C-204's
shape with a much smaller blast radius, and it is closed with a `Sec-Fetch-Site: cross-site` refusal.
**That check is defence in depth, not the control.** The controls are that the route is absent
without `--dev` and that the host is loopback-only; a header absent (a command-line client) is
allowed, which is what keeps `curl` usable and is therefore also the check's limit.

## What is still out of scope

- **A second request path.** Unchanged, and the reason `connectors-app` superseded
  `connectors-proxy`. Untouched by tenancy.
- **Publication.** `publish = false` (`crates/connectors-api/Cargo.toml`); the publish closure stays
  four crates ([C-190](../stories/C-190-publish-catalog-pack-secrets.md)). The amendment is about
  deployment, not crates.io.
- **Being flux's execution path.** flux loads `.flux` modules from `~/.flux/flows`. A change that
  routes flux's own connector traffic through this service inverts the project.
- **Inbound delivery.** No webhook endpoint, no relay. Inbound stays compiled, not hosted.
- **Forwarding, and any caller-named destination or credential.** The deputy line, above.

## Risks

- **The gate is prose.** Every item in §"The bind" is enforced by review, and the thing it guards is
  a one-line change. The mitigation that would match `connectors-app`'s own standard is a test that
  fails when the bind is not loopback while `tenant_of()` is a constant — mechanical, cheap, and
  worth more than this paragraph.
- **The charter now runs ahead of the code**, which is the failure mode the amendment was written to
  end and could reintroduce in the other direction. §"What the host actually is today" is the
  correction and needs re-measuring, not re-reading, when cited.
- **~~`MemoryStore` becomes the deployed store by default.~~ Closed by C-207, and replaced by its
  successor.** The risk was that credentials surviving a restart would arrive as a feature request
  before the design for holding them at rest did. It did not: the store is a choice, its default is
  a `0600` file, and there is no fallback to memory. **The successor risk is that the file reads as
  more than it is.** It is plaintext under a file mode — no encryption, no protection from `root` or
  from a backup — and the mitigation is that every surface an operator meets says so in those words:
  the module documentation, the README, and the startup banner assembled from the store itself so it
  cannot describe a different one. The thing to watch for is a later change that softens any of those
  three, or a deployment that reads "persistent" as "safe to put on a shared host".
- **"Multi-tenant" reads as permission for the general shape.** It is permission for *this* shape:
  the caller names an operation and nothing else.
