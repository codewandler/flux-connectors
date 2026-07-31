# Design: connector surfaces — what a connector can bring to flux

**Status:** accepted (descriptive — it records what *is*, and names what does not exist) ·
**Pillar:** Spec · **Amends:** [vision.md](../vision.md)'s "auth + operations + quirks"

> Every `path:line` below was read in this repository on **2026-07-31**, against a catalogue of
> **43 providers / 242 operations / 50 services** (`jq '.providers|length'` and
> `jq '[.providers[].operations|length]|add'` over `web/public/catalog.json`). Symbol names are
> stable; line numbers are not — re-grep by symbol rather than trusting a number that does not land.
> Note that `AGENTS.md`'s "Snapshot: v0.5.0 — 19 providers and 110 curated operations" predates the
> second provider fan-out and is no longer the measured figure.

## Why

[vision.md:30](../vision.md) still defines a connector as *"what remains once you stop hand-writing
the part a machine can derive: **auth + operations + quirks**"*. That sentence was true of an IR with
three surfaces. `Connector` (`crates/connector-spec/src/ir.rs:760-849`) now has **sixteen fields**,
and the three the vision names are not even the interesting ones any more: `quirks` reaches almost
nothing, and `auth` reaches neither the module nor the manifest.

There is no single place that answers *"what can a connector bring to flux?"* — the answer is spread
across eight design docs, each true about its own surface and silent about the rest. Someone
proposing a new surface has nothing to compare it against, and someone consuming the catalogue has no
way to know which declarations are load-bearing and which are inert.

This document is that answer, and its most useful content is the negative half: **six surfaces reach
no artifact at all.** They load, they validate, they move `ir_sha256`, and nothing downstream can see
them.

## The framing that replaces "a set of operations"

> **A connector declares what a vendor can do in both directions, and what an operator must supply
> to use it.**

Three clauses, each doing work:

- **what a vendor can do** — the capability half: `operations`, `events`, `graphs`.
- **in both directions** — outbound (flux calls the vendor) and inbound (the vendor calls flux), the
  distinction [inbound-events.md](inbound-events.md) established and `channels` compose.
- **what an operator must supply** — `auth`, `config`, and the `verify` that proves they got it
  right. This is the half "auth + operations + quirks" collapses into one word, and it is the half a
  hosted product spends most of its code on.

Everything else in `Connector` is identity (`id`, `authority`, `api_version`, `vendor`, `base_url`,
`description`), a default (`default_auth`), or bookkeeping (`provenance`). Those seven are not
surfaces; they are what a surface is addressed and versioned by.

## Members share one name namespace per service

The five surfaces that name something callable or matchable — `operations`, `events`, `channels`,
`config`, `graphs` — are **members of a service**, and they share **one namespace within it**. This
is not aspirational; it is enforced. `Connector::member_names_of`
(`crates/connector-spec/src/ir.rs:1057`) returns all five kinds concatenated precisely so that a
caller cannot get the rule wrong by checking three lists and forgetting the fourth, and the loader
refuses a duplicate across kinds.

The reason is recorded at that symbol and is worth repeating here because it is the constraint any
new surface inherits: all of them project into the same address space (`com.slack.api:v1#slack` must
denote one thing, and an `Oip` carries no kind discriminator), and all of them project into flux's
declaration namespace. A configuration field is not addressable in the calling sense — nothing calls
it — and it is in the namespace anyway, because it shares the *host's*.

**A sixth kind would have to join that namespace or justify why not.** That is the first question to
ask of any proposal to extend this list.

## The table

Artifacts, abbreviated: **F** = `connectors/<provider>[-<service>].flux` · **M** =
`connectors/<provider>[-<service>].connector.toml` · **R** = the embedded Rust catalogue
(`crates/catalog/src/generated/<provider>.rs`) · **J** = `web/public/catalog.json` · **T** = the
`ToolSpec` projection in `connector-pack`.

| surface | TOML spelling | what it emits | what consumes it | status |
|---|---|---|---|---|
| **operations** | `[[operations]]` | **F** one `op` each · **M** ids only · **R** + **J** full rows · **T** one `ToolSpec` each | flux's module loader; the explorer; `connector-pack` | **complete** — the only surface that reaches every artifact |
| **services** | `[[services]]` | **F**+**M** the *emission unit*: one module and one manifest per service · **J** a `services[]` block | the build's file split; the explorer | **complete**, with one gap: **R** has no service field at all (`crates/catalog/src/lib.rs:264-291`) |
| **auth** | `[[auth]]`, `[[default_auth]]` | **R** + **J** only · applied at execute by `connector-pack` | `connector-pack`; the explorer | **not in F, not in M** — that is [C-10](../stories/C-10-auth-injection-and-manifest.md), still `ready` |
| **events** | `[[events]]` | **M** minus `schema`/`when` · **J** with both | a host registering subscriptions | **complete**; the omission is deliberate, see below · absent from **R** |
| **channels** | `[[channels]]` | **M** + **J** · **nothing into F, by design** | a host; the explorer | **declaration complete, no runtime.** The adapter is [C-118](../stories/C-118-connector-channel-adapter.md) |
| **config** | `[[config]]` | **nothing** | **nothing** | **IR-only.** Validated, hashed, invisible — [C-87](../stories/C-87-configuration-codegen.md) |
| **graphs** | `[[graphs]]` | **nothing** | **nothing** | **lowering exists and is uncalled**; no provider declares one |
| **roles** | `roles` on `[[services]]` | **nothing** | **nothing** | **IR-only.** One variant, one provider — [C-121](../stories/C-121-llm-catalogue-role.md) |
| **verify** | `verify` (connector level) | **nothing** | **nothing** | **IR-only.** Declared by 20+ providers |
| **quirks.pagination** | `[operations.quirks.pagination]` | **nothing** | **nothing** | **IR-only.** Declared by real providers |
| **quirks.rate_limit** | `[operations.quirks.rate_limit]` | **nothing** | **nothing** | **IR-only**, and **declared by no provider at all** |
| **quirks.error_envelope** | `[operations.quirks.error_envelope]` | **F** as *prose appended to the op's description* — nothing else | the model reading the tool contract | **prose only** |

### Where each cell was read

- **operations** — `crates/connector-cli/src/seam.rs:285-296` (one `emit_operation` per operation),
  `:307-322` (the module), `:381` (manifest ids); `crates/connector-cli/src/catalog.rs:90-98`,
  `:280-313`; `crates/connector-cli/src/site.rs:479-486`; `crates/connector-pack/src/spec.rs:110-113`.
- **services** — `crates/connector-cli/src/pipeline.rs:213-217` (one module per service);
  `crates/connector-flux/src/op.rs:700-704` (`base_url_of(&operation.service)` is what binds a
  module's `base`); `crates/connector-cli/src/seam.rs:365-378`; `crates/connector-cli/src/site.rs:311-332`.
- **auth** — `crates/connector-flux/src/op.rs:57` records that no credential is emitted, and
  `grep -l Authorization connectors/*.flux` is empty. The manifest's whole wire shape is
  `crates/connector-cli/src/seam.rs:362-386` — twelve fields, none of them auth — and the emitted
  header says so out loud: *"Auth and the `http_hosts` allowlist land in C-10."* Both catalogues do
  carry it: `crates/connector-cli/src/catalog.rs:396-415`,
  `crates/connector-cli/src/site.rs:737-771`. `connector-pack` resolves and places it at
  `crates/connector-pack/src/tool.rs:213`.
- **events / channels** — `ManifestEvent` at `crates/connector-cli/src/seam.rs:440-455`,
  `ManifestChannel` at `:454-486`; `crates/connector-cli/src/site.rs:141-162`, `:167-201`. Neither
  reaches the Rust catalogue: there is no `EVENTS` static anywhere under
  `crates/catalog/src/generated/`.
- **config** — read nowhere outside the loader (`crates/connector-spec/src/provider.rs:372`, `:482+`)
  and the IR helpers (`crates/connector-spec/src/ir.rs:1023`, `:1030`). Every occurrence in an
  emitter crate is a `config: Vec::new()` test fixture. `connectors/freshdesk.connector.toml` is the
  demonstration: `providers/freshdesk.toml` declares `[[config]]`, and the manifest is nine lines
  with no trace of it.
- **roles** — declared at `crates/connector-spec/src/ir.rs:596-610`, checked at
  `crates/connector-spec/src/provider.rs:1806-1830`, union derived at `ir.rs:951`. `ServiceEntry`
  (`crates/connector-cli/src/site.rs:311-332`) has no `roles` field, and a key-walk of
  `catalog.json` at every depth finds none. Measured directly:
  `providers/anthropic.toml:195` declares `roles = ["llm_catalogue"]`, and
  `jq '.providers[]|select(.id=="anthropic").services' web/public/catalog.json` returns two service
  objects with seven keys apiece, none of them `roles`.
- **verify** — loaded at `crates/connector-spec/src/provider.rs:373` and validated at `:663-689`
  (must exist; must not be `high` or `destructive`). No emitter reads it.
  `grep -c '^verify' providers/*.toml` finds it on 20+ providers; `grep verify connectors/*.connector.toml`
  finds it on none.
- **graphs** — see the section below.
- **quirks** — the struct is `crates/connector-spec/src/ir.rs:415-425`. `pagination` has no reader
  outside the loader despite `providers/zendesk.toml:158`, `providers/twilio.toml:203` and
  `providers/babelforce.toml:216` declaring it. `rate_limit` has no reader **and no declaration** —
  every occurrence in `providers/` is a comment explaining why the vendor's published limit was *not*
  encoded (`providers/hubspot.toml:107`, `providers/intercom.toml:117`,
  `providers/airtable.toml:165`). `error_envelope` is read at
  `crates/connector-flux/src/op.rs:637-660`, which appends a sentence to the operation's
  `description` — visible at `connectors/anthropic-admin.flux:6`.

### The `error_envelope` cell deserves a second look

It is the only quirk any emitter reads, and what it emits is **English, into a description**. That
prose then rides along wherever the emitted Flux travels: into the Rust catalogue through
`include_str!`, into `catalog.json`'s `flux` string (`crates/connector-cli/src/site.rs:444`), and
into `ToolSpec.description`, because `connector-pack` takes the description from the *parsed
declaration* rather than the catalogue column (`crates/connector-pack/src/spec.rs:22`, `:30-40`).

It does **not** reach the `description` column of either catalogue, which uses the raw
`Operation::description` (`crates/connector-cli/src/catalog.rs:73-76`,
`crates/connector-cli/src/site.rs:408`). So the same operation carries two different descriptions
depending on which surface you read it from — verified on `anthropic-organization-get`, whose
`catalog.json` description ends at "…which organization a key resolves to" while its `flux` string
continues with the envelope sentence.

That is not a defect to fix here, but it is a fact worth writing down before someone builds a
consumer that assumes the two agree.

## The deliberate omissions, which are not gaps

Three cells above say "no" for a recorded reason, and conflating them with the six dead surfaces
would be the misread this document exists to prevent.

- **`channels` and `events` emit nothing into `.flux`.** flux lifts `op` declarations only; `channel`
  and `trigger` are Program members an operator writes. The tempting wrong output is an event dressed
  up as a pollable op, and `AGENTS.md`'s member contract refuses it on sight. The declaration reaching
  the manifest and the catalogue is the *whole* intended output.
- **An event's `schema` and `when` are dropped from the manifest** (`crates/connector-cli/src/seam.rs:346-352`)
  because TOML has no null and a half-populated table is worse than an absent one. `catalog.json` is
  JSON and carries both (`crates/connector-cli/src/site.rs:159`, `:161`), so nothing is lost — the
  omission is per-format, not per-surface.
- **`auth` is absent from `.flux` on purpose and absent from the manifest on schedule.** The module
  never names a credential because [auth-seam.md](auth-seam.md)'s whole argument is that acquisition
  in Flux would expose raw tokens in model-visible symbols. The *manifest*'s absence is different: it
  is C-10, unstarted, and the generated header says so in every file.

  One credential name does leak into the manifest today, and it is worth knowing about: a channel's
  HMAC block carries `secret = "slack.signing_secret"`
  (`crates/connector-cli/src/seam.rs:519`, `:586`; visible at
  `connectors/slack.connector.toml:77`). That is a *reference into* `connector.auth`, not the auth
  declaration — no scheme, no env var, no placement, no prefix.

## Graphs: the lowering exists and nothing calls it

This is the strangest row in the table and the one most likely to be misdiagnosed as unfinished work.

`connector_flux::emit_graph` is **complete**. It is defined at
`crates/connector-flux/src/graph.rs:125`, exported at `crates/connector-flux/src/lib.rs:29`, and
covered by `crates/connector-flux/tests/graph_emitter.rs` — seventeen call sites, most of them
asserting a specific *refusal*: a cycle, a value escaping a region, a gate exporting a symbol, a
composite literal that cannot round-trip, an unbound placeholder. The design
([flow-graph.md](flow-graph.md)) is written and the refusal discipline it specifies is implemented.

`crates/connector-cli/src/seam.rs` never mentions it. The loader still validates graphs
(`crates/connector-spec/src/provider.rs:1256-1280`) and they are in the hash domain
(`crates/connector-spec/src/ir.rs:1314-1317`). **And no provider declares `[[graphs]]`** — the single
match in `providers/` is a comment listing key names at `providers/stripe.toml:44`.

So the surface is not blocked on a missing capability. It is blocked on two lines in the pipeline and
on nobody having wanted one. Those are very different problems, and the second is the one that should
be answered first: a graph emitter wired into a build that emits zero graphs proves nothing.

## Six surfaces reach no artifact

`config`, `roles`, `verify`, `graphs`, `quirks.pagination`, `quirks.rate_limit`.

The honest summary is that **more than a third of the declared surface area of a connector is
currently unobservable to any consumer.** A provider author can spend real effort on a configuration
surface — labels, help text, formats, examples, `binds` targets, all of which the loader checks
rigorously — and produce exactly zero bytes of output.

Each has a different reason and a different fix, and they should not be batched:

| surface | why it stops | what would move it |
|---|---|---|
| `config` | the emitters were never extended | [C-87](../stories/C-87-configuration-codegen.md) — `ready`, and it carries a breaking `SCHEMA_VERSION` decision |
| `roles` | C-120 landed the declaration; the projection is a separate story | [C-121](../stories/C-121-llm-catalogue-role.md) — `ready` |
| `verify` | no story owns it; it rides along with the config surface | C-87's acceptance names it |
| `graphs` | the emitter is not wired, and there is no input to wire it to | a provider that wants one |
| `quirks.pagination` | no consumer was ever designed | undecided — see below |
| `quirks.rate_limit` | no consumer, **and no provider declares it** | probably deletion, not implementation |

`quirks.rate_limit` is the one that should give a reader pause. It is a field in the IR, in the hash
domain, and in the loader's `deny_unknown_fields` contract, and after 43 providers **not one author
has used it** — three of them wrote comments explaining why they deliberately did not. A field with
no producers and no consumers is not an unfinished feature; it is a shape the model does not need,
and the cheapest correct action is to say so.

## The hash domain disagrees with the artifacts, and that is the tell

`Connector::hash_domain` (`crates/connector-spec/src/ir.rs:1257`, projection at `:1281-1318`)
includes **fifteen of the sixteen fields** — everything except `provenance`, which is excluded for
the recorded reason that it describes where bytes came from rather than what was compiled from them.

Read that against the table and the mismatch is stark: `config`, `verify`, `graphs`, `roles` (via
`services`) and both dead quirks are all in the hash domain. Editing any of them moves `ir_sha256`,
churns `connectors.lock`, and changes **zero artifact bytes**.

The hash domain's own doc comment makes the claim explicit — it is *"the connector's compiled
meaning, not the module's bytes"* — and that claim is defensible for `events` and `channels`, which
reach two artifacts. It is a **statement of intent** for the six that reach none: the hash says these
are compiled meaning, and nothing compiles them.

That is the cleanest available measure of the gap this document reports, and it is why the
`HashDomain` struct's exhaustive destructuring (`ir.rs:1281-1318`, and the comment at `:1245`
explaining that a new field is a compile error until someone classifies it) is the right tripwire in
the wrong place. It forces an author to decide whether a new surface is *meaning*. Nothing forces
anyone to decide whether it is *output*.

## What this document does not decide

- **Whether a dead surface should be emitted or deleted.** Six surfaces reach nothing; at least one
  (`quirks.rate_limit`) probably should not exist. Each is its own story.
- **Whether the list should grow.** A new surface must join the per-service namespace
  (`member_names_of`), state which artifacts it reaches, and answer the `HashDomain` destructuring.
  This document gives a proposal something to be compared against; it does not pre-approve one.
- **The `.flux` module's primacy.** It remains the human-readable contract and the artifact flux
  loads. Nothing here argues for a second execution format, and
  [connector-tool-pack.md](connector-tool-pack.md) records why the pack is an additional surface
  rather than a replacement.
