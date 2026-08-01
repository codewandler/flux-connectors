# flux-connectors — roadmap & status

The big picture: what's delivered, what's next, and the epics that group related stories. The
operational detail lives on the [board](stories/README.md) (generated from story frontmatter); this
document is the hand-written narrative around it.

## Status

_As of 2026-08-01:_ the compiler is built, the catalogue is real, and **131 of 237 stories are
closed** across eight releases up to **v0.8.0**. `cargo run -p connector-cli -- diff` reports
`557 artifacts up to date (53 providers checked)` — 60 services, 299 curated operations, 8 events and
2 channel bindings. Twenty epics carry the work, not the single **connectors-v1** this section once
named; ten of them have a narrative below and the rest live on the board.

Working end to end: provider TOML → IR → a Flux module, a capability manifest, one rendering per
operation, an embedded Rust catalogue and the published `catalog.json` behind the explorer — hermetic,
offline and byte-reproducible, with `connectors.lock` recording what produced what. Beside the
compiler sit two **host libraries**: `connector-pack` projects every operation onto a flux `ToolSpec`,
assembles its credential in Rust and hands a registry declarations; `connector-secrets` resolves a
credential address to a value over an in-memory or Vault-backed store.

**The external dependency this section used to name is gone.** flux's `$auth` seam was the critical
path for a live call; C-114, C-115 and C-116 dissolved it by assembling auth inside the Tool pack, so
the whole-value `{"$secret"}` marker never has to grow a prefix or an encoder and no flux release
gates the milestone. [designs/auth-seam.md](designs/auth-seam.md) is kept as the composite-path
design. What still waits on flux is narrower: the form and query **encoder** (upstream `L-101`), which
is what keeps `zendesk-ticket-search` and every `form` body non-functional.

A live call is no longer gated. `codewandler-flux-web` 0.41 supplies the `http.request`
implementation `Egress` takes as a constructor argument, and `crates/connectors-api` is the host that
binds it and runs the loop — the first thing here that calls anyone (C-202, C-203). See
[designs/connectors-api.md](designs/connectors-api.md). The loopback-only narrowing in
[designs/connectors-app.md](designs/connectors-app.md), which was how C-34 resolved as
**yes-narrowed**, is superseded by the owner-directed charter amendment in C-201 — superseded, not
deleted: the `Egress` analysis and the slice-1 sequence it rested on are still current and still
cited.

What the host is *today* is narrower than what the charter now permits, and the gap is stated in
[designs/connectors-api.md](designs/connectors-api.md)'s measured table rather than left to
inference: the bind is still `127.0.0.1` with no flag. The other two are closed — C-204 replaced the
constant `"local"` tenant with one that comes from a Google-backed session, and C-207 replaced the
in-memory credential store with a 0600 file the host refuses to open when it is wider. Both are
recorded, with transcripts, in `crates/connectors-api/README.md`.

The largest gap is not a blocker but a hole: **six declarable surfaces reach no artifact** —
`config`, `verify`, a service's `roles`, `quirks.pagination`, `graphs` and `quirks.rate_limit`. The IR
models each and the loader validates it, and then neither the manifest nor the published catalogue
carries it, so a host cannot render a connector's settings page or find its "Test connection"
operation for connectors that declare both. `AGENTS.md`'s *Intentional gaps* has the table.

## Delivered

The itemized history is [CHANGELOG.md](../CHANGELOG.md); this is its shape.

- **v0.0.1** — the Cargo workspace, the connector IR with deterministic serialization, the
  provider-TOML front-end, the Flux op emitter through `flux_lang`'s AST, the `build`/`diff` CLI with
  its offline guarantee proven three ways, `connectors.lock` with an explicit hash domain, and the
  first three providers (zendesk, freshdesk, babelforce).
- **v0.1.0** — the public VitePress site, the provider and operation explorer, `catalog.json` as a
  fourth backend over the same IR, and a README image highlighted by flux's own `flux_lang::highlight`.
- **v0.2.0** — OpenAI, GitHub and Slack; the rewritten root README.
- **v0.3.0** — **services** as the middle addressing level (C-49), the first provider fleet
  (Jira, Google Workspace, OpenRouter, Zoom, Airtable, Sentry, Asana, HubSpot, Intercom, Shopify), and
  C-54's deletion of five hand-maintained provider lists.
- **v0.4.0** — the configuration surface (C-86), channel bindings (C-82), the flow graph (C-94),
  credential addressing (C-90), the auth archetype matrix (C-22), the core-catalogue projection, and
  an explorer that renders outside the prose column.
- **v0.5.0** — the **Tool pack** (C-114/C-115/C-116): a catalogue operation projected onto a
  `ToolSpec`, gated individually by flux's permission envelope, authenticating from a bound
  `CredentialStore` with the secret registered before the request is built. Plus `connector-secrets`
  (C-91), composed `input_schema`s (C-125), events and channels reaching the manifest and catalogue
  (C-83), service roles (C-120), verification conformance against real vendor vectors (C-60), and
  C-104 making whole-catalogue artifacts coordinator-owned — the change that let provider stories run
  in parallel at all.
- **v0.6.0** — the second provider fleet, run in waves rather than one at a time: past forty
  vendors, with each connector chosen for the modelling question it forces rather than the row it
  adds. Plus `body_encoding = "form"` (C-144), a measured floor under response-shape coverage (C-126),
  `AuthScheme::Header { prefix }` (C-184), so a credential can sit inside a header value it does
  not wholly occupy, and C-197's fix for two services of one connector collapsing into one
  configuration value.
- **v0.7.0** — `crates/connectors-api`, the reference host (C-202, C-203), and **the first byte this
  repository ever sent to a vendor**. Google sign-in, accounts and sessions (C-204); an installable
  tenant scope (C-187); Bitbucket, Discord, Confluence, New Relic, Mailchimp, Klaviyo, Supabase and
  Resend. Also the first crates.io publish of the four-crate closure, at 0.7.0.
- **v0.8.0** — credentials that survive a restart (C-207), a dev sign-in so the host runs without a
  Google registration (C-234), a `User-Agent` on every outgoing request (C-223), a checked MSRV
  (C-213), and C-186's requirement that a repeatable write state the condition it depends on.

## Publishing

**Publishing to crates.io is CI-only, and nobody runs `cargo publish` by hand.** A release is a
consequence of pushing a `vX.Y.Z` tag; `.github/workflows/crates-io.yml` publishes the closure
idempotently from a single `CARGO_REGISTRY_TOKEN` secret, so a run that trips the new-crate rate
limit resumes rather than stranding a half-published set that cannot be withdrawn. See
[AGENTS.md § Publishing contract](../AGENTS.md) and
[designs/crates-io-publishing.md](designs/crates-io-publishing.md).

**The closure is four crates, and which four changed at C-407.** `connector-secrets` re-exports
`CredentialRef`, so whichever crate owns that vocabulary is in its public API and must ship or
nothing outside this workspace resolves. Until C-407 that crate was `connector-spec` — the connector
IR, both front-ends, validation and the lockfile writer, 11,832 lines of compiler shipped so that a
credential address would resolve. This paragraph used to record that as a fact of life; it was a
dependency-direction problem, and extracting the vocabulary into `connector-address` ended it. In
dependency order:

```
codewandler-connector-address → codewandler-connector-catalog
  → codewandler-connector-secrets → codewandler-connector-pack
```

The order is **derived from the manifests** by `scripts/publish-crates-io.sh --print-order`, never
hand-listed, and `crates/connector-cli/tests/publish_closure.rs::no_machinery_crate_is_published`
fails if a machinery crate finds its way back in. `connector-spec`, `connector-flux` and
`connector-cli` are now all unpublished, which is the direction this family was already heading: the
repository ships **data and address vocabulary**, not the machinery that produces them.

`codewandler-connector-spec` 0.7.0 and 0.8.0 are already on crates.io and cannot be withdrawn. C-407
stops the *next* version shipping; it does not undo the two that went out.

The `codewandler-` prefix matches the flux family and was chosen deliberately: bare `connector-*`
names are a contested namespace, and `connector-cli` is already taken on crates.io by an unrelated
project. Package names are decoupled from crate names by `[lib] name`, so `use catalog::` and
`use connector_spec::` are unaffected.

**All four are published, and the order was deliberate.** The closure first went out at **0.7.0** on
**2026-07-31** and is at **0.8.0** as of the same day, in the dependency order above. Everything the ordering waited
on had landed by then: [C-197](stories/C-197-config-collapses-across-services.md),
[C-92](stories/C-92-authorities-for-every-provider.md), and
[C-192](stories/C-192-flux-0-41-bump.md) — a consumer must link exactly one flux-runtime, because
`connector-pack` hands out `Arc<dyn Tool>` and two engine versions are two incompatible types — plus
one proven live call, recorded in `crates/connectors-api/README.md`.

*A correction worth keeping, because it was the stated reason for this ordering:* C-197 was expected
to be a **breaking** change to `catalog::Operation`, and therefore to have to precede any publish of
`connector-catalog`. It measured otherwise — the struct is already `#[non_exhaustive]`, so an
external consumer can neither construct it with a struct literal nor destructure it exhaustively, and
a new field cannot break them. The genuinely breaking surface was **`connector-pack`**
(`ConfigStore::get` gained a parameter, `Error::MissingConfig` a field). So the sequencing was right
and the reason was wrong: it was never `connector-catalog` that was at risk.

## Next

The ranked, actionable form is the **Next** list on the [board](stories/README.md). In short: close
the surface gap so a host can read what a connector already declares, stand up the reference host that
proves the seams end to end, and keep the fleet growing in parallel waves.

## Epics

An **epic** is a themed group of stories with a shared design doc. Stories join an epic via the
`epic: <slug>` frontmatter field, where `<slug>` matches a design doc at `docs/designs/<slug>.md`.

### The connectors datasource — the catalogue, queryable from a session

A flux session cannot ask "which connector can do this?". The catalogue is published and unreachable
from inside a running flow, so an agent either has every connector operation registered as a tool or
none of them.

The scaling argument is why this matters now. The Tool pack registers **one tool per operation** — 97
when this epic was filed, **232** as of 2026-07-31, and the fleet stories keep multiplying it — and
every one is model-facing surface: schema in the context window, a name to disambiguate, a chance to
pick wrong. A datasource is a fixed handful of operations whether the catalogue holds 97 or 970. The two are complementary: discover through the
datasource, invoke through the pack.

The seam is flux's and already built — `LiveDatasource`, `try_register_live_datasource`, and
`ClientBuilder::try_with_live_datasource`, which is the same call a host uses for the pack. The
catalogue is already shaped for it, which is what the addressing work bought: the `oip` is a stable
record id, and a channel binding's link to its reply operation is C-82's composition made traversable.

Done looks like search, get, list, relation and batch-get answered offline from the compiled-in
catalogue — with search good enough to act on, since one that returns the wrong connector confidently
is worse than none. What it must **not** become is an HTTP service: that is the proxy charter question
C-34 already gates. See [connectors-datasource.md](designs/connectors-datasource.md).

### Authentication as a connector surface — a login that cannot leak

A connector can declare which credential it needs, but not a login you can *trigger*. This epic adds
one — `oauth2.login(grant: password, …)` and its sibling grants, as members of a service claiming an
`authentication` role — and gives `OAuth2Spec` its first real consumer; C-88 already records that it
is a landed type no shipped provider uses.

It is also the most dangerous operation shape this repository has modelled, and the design is mostly
about that. An operation's result becomes a session value: bound to a symbol, interpolatable,
readable by the model that called it, printable to a log. A login returns a bearer token, so the
naive shape violates the requirement by succeeding.

Redaction cannot fix it, and C-79 already proves why in the concrete: Zoom's `start_url` carries a
host-privileged token the redactor cannot see. Redaction matches values it was already told about,
and a token minted by *this* call is unknown until after it arrives.

So done looks like a structural answer, not a filter: a credential-producing operation's declared
output is a **handle** — a `CredentialRef` naming where the value was stored — while the token goes
from the HTTP response straight into the bound store and never enters the session. A caller can use a
credential it can never read, and an operation whose declared output would expose the secret is
refused at load. See [authentication-surface.md](designs/authentication-surface.md).

### babelforce IVR v2 — atomics, not call modules

babelforce's IVR has two layers: primitives in `internal/modules/` (`audioplayer`, `read`,
`switchnode`, `dial`, `recording`, `acd`) and call modules composed over them in `flows/*.yaml`.
`simpleMenu` is `audioplayer` + `read` + `switchnode` welded together — so publishing call modules as
operations would freeze seventeen combinations instead of exposing six composable parts. The epic
exposes the atomics as a `service = "ivr"` at `api_version = "2"`, with the reverse direction as
events.

Done looks like: the atomics and their events in the catalogue, no call module published as an
operation, and the two different "invite" meanings named apart — the ACD inviting an *agent* to take a
queued call is not the inbound call arriving.

The open question is deliberately unanswered rather than assumed. A flow YAML *is* a graph, but its
edges are `goto`s, and the flow-graph model refuses cycles because Flux has none — a menu that
re-prompts on invalid input jumps backwards. An IVR flow is a state machine executed by *babelforce's*
engine, which is a third case beyond "this repo compiles, flux executes". C-132 decides whether
composed templates belong here at all; nothing else in the epic waits on it. See
[babelforce-ivr-atomics.md](designs/babelforce-ivr-atomics.md).

### Provider fleet 2 — shipped in parallel

The first fleet (C-69–C-78) is fully drained and every connector in it shipped one at a time. That
was not a staffing limit: **`crates/catalog/src/generated.rs` carries two hand-maintained lists that
every provider story appends to**, so any two branches conflict on one file and integrate serially
however many implementors run. `web/public/catalog.json` has the same shape and already solves it
correctly — it is emitted only on a full run, and a scoped build leaves it untouched.

C-104 applies that rule to the one list C-54 left behind, which makes provider write sets pairwise
disjoint and moves the wave size from one to whatever disk allows.

The five connectors that follow are each chosen for what they force the model to confront rather
than to add a row: Stripe is the second vendor behind the webhook HMAC matrix, Notion cannot ship
until a provider can declare a constant header, Microsoft Graph asks whether a service is a real
addressing level or was Google's host problem in disguise, Twilio puts one value in both a
credential and a path, and Linear asks whether a GraphQL vendor can be a connector at all — with a
documented refusal an acceptable answer.

**Done looks like:** a wave of provider branches that merge without touching each other, and a
catalogue that grew by five without anyone editing a shared list by hand.

### The host's explorer — an operator console

There are two explorers. The public one at `web/` is fifteen Vue components and 2,434 lines; the one
the operator actually works in is a single 355-line HTML file compiled into the host binary. The
owner ran the app on 2026-07-31 and reported the difference, and the cause is worth recording because
it is not neglect: [C-203](stories/C-203-connectors-api-skeleton.md) scoped that page as *"no UI
beyond what proves it"*, every change since has been a rider on a backend story, and
[designs/explorer-ux.md](designs/explorer-ux.md) — the only document here that reasons about explorer
layout, density and filters — is scoped exclusively to `web/` and never mentions it. No story ever
asked for more. Design: [designs/host-explorer.md](designs/host-explorer.md).

The constraint that shapes every option: these are not two attempts at one thing.
[C-147](stories/C-147-explorer-runs-an-operation.md) forbids the public site collecting a credential
or implying a live call, and this surface exists to do both. So credential capture and execution can
never move into a shared component — convergence means sharing the browsing half and keeping the
operating half in the host. [C-142](stories/C-142-detach-the-explorer-components.md) already detached
the components from VitePress for exactly this, and their README marks the page tier as the one a
host may reasonably decline.

**Done looks like:** an operator can find one connector among fifty-three, see at a glance which need
setup, run an operation against a readable response, and the page they do it on shares its components
and its visual language with the page they read.

### The explorer at fleet scale

The public explorer was designed against six providers and twenty-five operations. When this epic was
filed it indexed sixteen providers, eighteen services and eighty-eight operations; as of 2026-08-01 it
is **53 providers, 60 services and 299 operations**, and the decisions that were right at a fraction
of the size are wrong at this one: VitePress's doc layout caps the content column at 688px, so
a `minmax(320px, 1fr)` provider grid renders exactly two columns and the five-control filter bar wraps.
Services — the middle addressing level C-49 established — are published in the catalogue and appear
nowhere in the UI. Design: [designs/explorer-ux.md](designs/explorer-ux.md).

The constraint that outlives the redesign: the explorer does **not** report "N of 299 operations
working". `works` is false for every operation for a reason that is *shared* — no host runs them here —
and a working-count headline would misrepresent the overwhelming majority that are exactly as designed
and waiting on that one thing rather than on anything of their own. (The shared reason has changed
since this was written: it was the `$auth` seam landing in flux, and it is now a reference host in this
repository. The argument against the headline is unaffected.) Presentation follows each issue's `scope`
instead — catalogue-wide once, per-provider on the card, per-operation as a badge.

**Done looks like:** a full-width explorer where sixteen connectors are visible without scrolling,
"every destructive Shopify operation" is a URL you can send someone, and the honest account of what
does not work is exactly as clear as it is today.

### Connectors v1 — spec to Flux

Prove the whole thesis on two real providers: a provider TOML plus a vendored vendor spec compiles
into a `.flux` module that flux loads as ops and exposes as LLM tools, with credentials resolved by
the host and never present in any artifact. Design:
[designs/connectors-v1.md](designs/connectors-v1.md); the pipeline itself is
[designs/connector-pipeline.md](designs/connector-pipeline.md).

**Done looks like:** `flux-connectors build && flux-connectors install`, then a `flux` session lists
`zendesk.ticket.show` and `anthropic.messages.create` among its ops and calls one successfully
against the live API.

### Inbound events — the reverse call direction

A connector today compiles **outbound** ops: flux calls the vendor. The other half is the vendor calling
**us** — a ticket updated, a call ended, a payment settled — and without it every connector-driven
automation has to poll, which is slower, costs quota, and cannot express "react when this happens." This
epic adds a declared `[inbound]` section: transport, verification, event identity, payload schemas, plus
generated subscription ops and a polling fallback for vendors with no webhook at all. Design:
[designs/inbound-events.md](designs/inbound-events.md).

Two findings shape it. First, **verification is a declarable matrix, not per-vendor code**: GitHub,
Stripe, Slack and Zendesk look bespoke but vary only over digest, encoding, the signed-string template,
and a tolerance window — one parameterized HMAC, which is what lets it be compiled rather than
interpreted. It is also the same request-dependent problem [C-50](stories/C-50-aws-services.md) hit from
the outbound side with SigV4, so one notion should cover signing and verifying rather than two.

Second, **inbound emits nothing into the `.flux` module.** flux lifts `op` declarations only from
`~/.flux/flows`, while `channel` and `trigger` are Program members an operator declares — so events land
in the manifest and the catalogue, and the emitter must refuse to dress an event up as a pollable op.
What crosses into flux is *parameters*, not code, and the blocking cross-repo fact is that flux's
`channel webhook` authenticates with an optional **static bearer token** and has **no signature path at
all** — so a vendor that signs but cannot send an `Authorization` header currently has no authenticated
route in. That seam is designed as [C-64](stories/C-64-design-verified-webhook-seam.md) and handed off as
paste-ready flux stories, following the C-16 precedent.

**Done looks like:** a real GitHub delivery and a real timestamped delivery (Stripe or Slack) verified and
routed to distinct triggers on a live flux — and a tampered body plus a stale timestamp each rejected with
**zero** deliveries, demonstrated rather than asserted.

### Unified auth

Every connector differs from its neighbours mostly in **how it authenticates**. Endpoints are
boring; credentials are where providers are irreducibly different and where a naive model runs out of
room fastest. This epic replaces flat scheme variants with three orthogonal axes — **source ×
acquisition × placement** — so a new provider archetype costs one value on one axis instead of a new
variant crossing all of them. Design: [designs/unified-auth.md](designs/unified-auth.md).

The three in-scope providers already break the flat model: zendesk and freshdesk need a base64 join
with different user halves, and babelforce needs a Bearer prefix with JWT planned. flux's four
`AuthScheme` variants become *presets* of the unified model rather than the vocabulary itself, which
is what keeps the `$auth` seam acceptable to flux instead of proposing a rival auth system.

**Done looks like:** the conformance matrix (C-22) expresses every real archetype we know of — raw
header, prefixed header, basic join, query key, AND, OR, unauthenticated, OAuth2, JWT — with no
provider-specific code, and the four flux presets round-trip exactly.

### Connector bundle

A connector is more than callable operations: it has schemas, metadata, branding and documentation.
This epic decides **where each piece lives** — and specifically how much belongs *inside* the `.flux`
file. Design: [designs/connector-bundle.md](designs/connector-bundle.md).

The constraint that decides it: `connectors/<name>.flux` is source that flux parses at session start,
so every byte in it is paid for by every session. Metadata therefore rides **synthetic pure
operations** (`describe`, `schema`) that ride the mechanism already there; icons ship as files beside
the module rather than base64 inside it.

**Done looks like:** a connector answers "what can you do, and with what shapes?" from inside a flux
session, and the bundle directory is produced deterministically and drift-checked like every other
artifact.

### The two milestone-1 providers

They are chosen to exercise different halves of the pipeline:

- **anthropic** — spec-driven with raw-header auth. Proves ingest → IR → codegen → registered op with
  no auth blocker in the way.
- **zendesk** — Basic auth and heavy patching. Proves the overlay layer, forces the auth seam, and
  tests the plugin-replacement claim directly against flux's `plugins/zendesk` (687 lines of Rust,
  reduced to one TOML).

### Generated connector tests — what can be derived, and what must never be

`crates/connector-flux/tests/` holds **52 `*_connector.rs` files totalling 22,455 lines**, roughly one
per shipped provider, and every new connector adds one by hand. The obvious reading is that this is
boilerplate a generator should write. The measurement says only partly, and the interesting part is
not the part you would generate.

Across those 52 files, 37 declare a `const PROVIDER`, 36 an env-var constant, 31 a `const OPERATIONS`,
29 a `const CREDENTIAL`, 26 a `const BASE_URL` — every one a second spelling of something the provider
TOML already states. But `slack_connector.rs` asserts one connector-specific property, that Slack
declares **no query parameter at all**, and explains why: Slack's ids are opaque strings, the emitter
interpolates query values with no percent-encoding (C-30), so a read expressed as a `GET` would ship
the same defect `zendesk-ticket-search` carries. Its header ends *"nothing else in the repository
would fail if someone converted a read to a GET, and the resulting connector would look tidier and be
broken."* No generator produces that, and a generator that replaced it would delete the only thing
standing between a tidy-looking change and a broken connector.

So the epic starts with a **measurement, not a generator** — and it is allowed to conclude "not worth
doing", which for a spike is a successful outcome. Two things follow from what it finds. The
mechanical bucket probably wants **deletion into a fleet-wide test** rather than generation: a test
whose expected value comes from the same IR that produced the artifact asserts that the generator is
the generator, and `flux-connectors diff` already checks all 557 artifacts byte-for-byte against
exactly that derivation. A disk-enumerating test in the `shipped_modules.rs` mould cannot drift and
covers providers not written yet. Separately, there is one genuinely new check available: the vendored
documents publish `example` blocks and, since C-4, resolved response schemas — two *independent*
statements by the vendor that nothing has ever compared.

Design: [designs/generated-connector-tests.md](designs/generated-connector-tests.md).

**Done looks like:** the question answered with a number — how many of the 22,455 lines restate the
provider file, how many are already covered fleet-wide, how many are reasoned claims that stay — and
then either the mechanical bucket is gone or the measurement says it was never the boilerplate it
looked like, written down so nobody re-opens it on a hunch.

### A connector's security posture — publish the facts, and be careful about the grade

Owner-asked on 2026-08-01: *"it would be great to have something like a security rating over a
connector — e.g. I could imagine Twilio's HMAC is quite safe compared to something using static tokens
which cannot easily be changed or rotated."* The intuition is right and the gap is larger than the
example. Measured the same day across 54 providers: the catalogue can say exactly **where** a
credential is placed — 31 bearer, 15 header, 3 basic, 1 **query** — and can say nothing at all about
**how long it lives, whether it can be rotated, or whether it can be revoked**. `Acquisition` has two
variants and its own documentation says `Minted` read as placement *is* `Static`, so a 30-day rotating
token and a permanent one are the same declaration. The axis the request turns on is the one axis that
does not exist.

The epic's first commitment is therefore **not** the rating. A grade computed from declarations
inherits the defect this repository keeps finding — C-430's Zoom and Postmark documented their hazard
precisely and returned the field anyway — and adds three of its own: it reads as a measurement while
being an opinion with arithmetic on top, it conflates inbound and outbound axes that a single letter
cannot separate, and it is gameable in the direction that improves the grade without improving the
connector. So the facts are published per axis, each traceable to a declaration the loader enforces,
and whether a composed grade ships is decided in the open once those facts exist.

One axis turned out to be load-bearing at runtime rather than descriptive: flux 0.47.1's credential
boundary **refuses** a response carrying credential-shaped material instead of redacting it, so an
operation that returns a token and does not say so does not leak — it fails.

Design: [designs/connector-security-posture.md](designs/connector-security-posture.md).

**Done looks like:** the catalogue answers, per connector and per credential, how long this credential
lives, whether it can be rotated, where it is placed, and whether inbound events are verified — each
distinguishing *unstated* from *stated poorly*, and nothing published claiming to describe a
connector's actual security rather than what it declares.
