# flux-connectors — roadmap & status

The big picture: what's delivered, what's next, and the epics that group related stories. The
operational detail lives on the [board](stories/README.md) (generated from story frontmatter); this
document is the hand-written narrative around it.

## Status

_As of 2026-07-30:_ the repository has just been scaffolded. Nothing is implemented yet — the
backlog, the two design records, and the Cargo workspace are the whole of it. The single epic,
**connectors-v1**, carries every story. The one external dependency is a change to `../flux`
described in [designs/auth-seam.md](designs/auth-seam.md); it is on the critical path and should be
designed and filed against flux's board before the codegen work finishes.

## Delivered

- _Nothing yet._ Itemized history lands in [CHANGELOG.md](../CHANGELOG.md) as stories close.

## Next

The ranked, actionable form is the **Next** list on the [board](stories/README.md). In short:
scaffold the workspace, design the auth seam early (longest lead time), build the spec crate, then
the codegen crate, then the CLI — and finish with two providers proven end-to-end against a live
flux.

## Epics

An **epic** is a themed group of stories with a shared design doc. Stories join an epic via the
`epic: <slug>` frontmatter field, where `<slug>` matches a design doc at `docs/designs/<slug>.md`.

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

### The explorer at fleet scale

The public explorer was designed against six providers and twenty-five operations. It now indexes
sixteen providers, eighteen services and eighty-eight operations, and the decisions that were right at
a third of the size are wrong at this one: VitePress's doc layout caps the content column at 688px, so
a `minmax(320px, 1fr)` provider grid renders exactly two columns and the five-control filter bar wraps.
Services — the middle addressing level C-49 established — are published in the catalogue and appear
nowhere in the UI. Design: [designs/explorer-ux.md](designs/explorer-ux.md).

The constraint that outlives the redesign: the explorer does **not** report "N of 88 operations
working". `works` is false for every operation until the `$auth` seam lands in flux, and a
working-count headline would misrepresent the eighty that are exactly as designed and waiting on one
shared seam. Presentation follows each issue's `scope` instead — catalogue-wide once, per-provider on
the card, per-operation as a badge.

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
