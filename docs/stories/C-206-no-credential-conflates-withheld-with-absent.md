---
id: C-206
title: "`no-credential` tells consumers a public endpoint is disabled for their protection"
pillar: Surfaces
status: done
design:
epic:
areas: [connector-cli, web]
note: "found by the C-133 implementor 2026-07-31 and verified: status.rs's own comment says reading the field directly 'would report freshdesk and a genuine ping endpoint the same way for opposite reasons' — and then `effective_auth` reports them the same way anyway. Blocks C-133 and C-157 identically"
---

# `no-credential` tells consumers a public endpoint is disabled for their protection

## Goal

Let the published catalogue distinguish **a credential deliberately withheld** from **a vendor that
requires no credential at all**, so a consumer filtering on operation status is not told a working
public endpoint is unavailable.

## What is wrong

`crates/connector-cli/src/status.rs:133-143` emits one issue for both cases:

```rust
if connector.effective_auth(operation).is_empty() {
    issues.push(Issue {
        code: NO_CREDENTIAL,
        summary: format!(
            "{} has no safe credential configuration for this operation yet. Live calls are \
             disabled rather than sending a credential outside Flux's secret protection.",
            connector.id
        ),
```

That sentence is **true of freshdesk** — a real API key exists, the current IR cannot mark it secret
because it occupies the Basic *username* position, so it is deliberately withheld and the resulting
401 is honest fail-closed behavior.

It is **false of any vendor whose endpoint is genuinely public.** Nothing is withheld, no credential
exists to withhold, and the unauthenticated call is the correct working call. The catalogue would
tell a consumer that a working public endpoint is disabled for their protection.

**The code already knows this, and says so, four lines above the bug** (`status.rs:128-131`):

> `effective_auth` rather than `Operation::auth`, always: an operation that declares nothing inherits
> the connector default, and one that declares an explicit empty list inherits nothing. Reading the
> field directly would report freshdesk and a genuine ping endpoint the same way for opposite
> reasons.

`effective_auth` correctly separates *inherits-the-default* from *declares-explicitly-nothing* — and
then both land in the same branch with freshdesk's wording regardless. The comment describes a
distinction the code does not go on to make.

## Why it is a story and not a wording fix

`NO_CREDENTIAL` is a **published contract token**. `status.rs:64-70` says so:

> Consumers switch on these, so they are part of the published contract
> (`docs/designs/catalog-json.md`) and are not renamed once shipped. A *new* code is additive; an
> existing one changing meaning is not.

So the fix is a new code alongside it, not a re-spelling of this one — and `works: false` is probably
wrong for the public case too, since the operation *does* work.

## Acceptance

- [x] **Failing-first test:** a connector declaring no credential because its vendor needs none is
      published differently from freshdesk, which declares none because its key cannot yet be held
      safely. The test must fail before the change with both rendering identically.
- [x] ~~A **new** issue code is added~~ **A new `notes` code, `no-credential-required`, is added**
      rather than `NO_CREDENTIAL` changing meaning, per the stability rule at `status.rs:64-70`.
      `NO_CREDENTIAL` keeps its freshdesk sense exactly.

      *Amended during implementation, because the literal wording is unsatisfiable jointly with the
      item below.* `works` is `issues.is_empty()` — a documented contract with a test on it — so a
      new **issue** code would have forced `works: false` onto an operation the next item requires
      to report `works: true`. That is the same lie `no-credential` was telling, one code further
      along. The code is therefore published in a new `notes` list beside `issues`: additive,
      switchable by a consumer, and it leaves `works` and every existing consumer untouched. The
      item's actual intent — *no existing token changes meaning* — is met exactly.
- [x] The IR can express the difference. Today "no `[[auth]]`" is one state carrying two meanings;
      an author must be able to say *"this vendor requires no credential"* as a positive declaration,
      distinct from *"a credential exists and we cannot hold it safely"*.
- [x] Decide and record whether a genuinely-public operation reports `works: true`. If it does, the
      explorer and `catalog.json` consumers see a working operation for the first time — which is a
      visible change worth stating deliberately rather than discovering.
- [x] `docs/designs/catalog-json.md` documents the new code, since it is a published contract.

## Progress

**2026-07-31 — implemented on `impl/C-206`.** The distinction is `no-credential-required`, and it is
a **note** rather than a fifth issue.

- **The IR needed no change.** `Operation::auth` already separates the two empties and documents
  them as OpenAPI does: `None` inherits `Connector::default_auth`, `Some(vec![])` is *explicitly
  none* — "a health or ping endpoint", in the IR's own words. So the positive declaration an author
  writes is `auth = []` on the operation, and `status.rs::declares_no_credential_is_needed` reads
  it. Nothing is inferred from an absence, which is the trap the dispatch named:
  `a_missing_credential_is_never_read_as_a_public_endpoint` pins it.
- **A note, not an issue, because `works` had to move.** `works == issues.is_empty()` is the
  contract every consumer already filters on, and a public operation *works*. Publishing the fact as
  a fifth issue would have kept `works: false` on an operation with nothing wrong — the same lie one
  code further along. So `Status` gained `notes: Vec<Note>` beside `issues`, and `works` is
  unchanged.
- **`works: true` is the recorded decision** (`docs/designs/catalog-json.md`, "A public operation
  reports `works: true`, deliberately"). No shipped operation is public today, so nothing in the
  committed catalogue moves — but a consumer treating `works: true` as unreachable will be wrong the
  first time C-133 or C-157 lands, and that is now stated rather than discovered.
**2026-07-31, rework after review — `notes` is always encoded.** The first cut gave it
`skip_serializing_if = "Vec::is_empty"` to keep `web/public/catalog.json` byte-identical. That was
wrong, and the reasoning behind it was wrong: `AGENTS.md:129-131` fences an implementor from
*regenerating* a whole-catalogue artifact, not from making one stale, and `AGENTS.md:155-171`
documents staleness as the normal case with a named procedure — report the red tests and stop, the
coordinator's full build at integration resolves them. So the saving was never needed, and what it
bought was a permanent hole in the document's oldest guarantee: `catalog-json.md` §Guarantees 1 says
every key is always present, and `site.rs:27-29` says nothing there uses `skip_serializing_if`. Both
sentences were false for all 242 shipped operations, none of which carries a note — the exception
was invisible in exactly the case a consumer meets first.

- `notes` is now unconditional, `[]` when empty.
  `a_status_with_no_note_still_carries_the_key` pins the whole published key list.
- The exception text is gone from `catalog-json.md`, so §Guarantees 1 stands unqualified again, and
  `site.rs:27-29` is true again on its own.
- **`optional_fields_are_null_rather_than_absent` (`site.rs`) is now derived rather than
  enumerated.** It named three fields — `body_schema`, `response_schema`, `user_suffix` — which is
  why it passed while a fourth grew a conditional key. It now renders the document twice from the
  same emitter, once with every optional absent and once with each present, and requires two objects
  at the same position to carry the same keys. Verified against the bug it missed: reintroducing the
  `skip_serializing_if` fails it with
  `` `$.providers[0].operations[0].status` publishes a different set of keys depending on its
  content `` — `["issues","works"]` against `["issues","notes","works"]`.
- **`web/public/catalog.json` is deliberately left stale.** Not regenerated, per the fence.

### For C-133 and C-157

Write `auth = []` on each unauthenticated operation. The declaration is **per operation**:
`Connector::default_auth` is a plain `Vec`, so `default_auth = []` and an omitted key decode
identically and a connector cannot make the statement once. Giving the connector field an `Option`
is a `connector-spec` change (loader, lockfile encoding, every provider file) and was out of this
story's write set.

### For the host — `crates/connectors-api` (not touched, another agent owns it this wave)

`api.rs:169` computes `connected: all_stored && !provider.auth.is_empty()`, which reports a
connector whose vendor needs no credential as **not connected** — the same conflation this story
closed in the catalogue, in the surface the owner actually looks at. A connector with no declared
credentials is either "you must supply something we cannot hold yet" or "nothing to do here", and
`!provider.auth.is_empty()` reads both as the first. Worth a story: the host can express it with the
fields it already has (`stored`, `address`), it just needs the third state.

## Notes

- **Blocks [C-133](C-133-provider-brave-talk-tokens.md)** (Brave Talk's handshake is unauthenticated)
  **and [C-157](C-157-ollama-model-catalogue.md) identically** — C-157 asked for this to be settled
  once for both, and this is where it gets settled.
- Freshdesk's case is recorded in `AGENTS.md` under Intentional gaps and must not be disturbed: its
  401 is the deliberate, correct outcome and stays exactly as it is.
- Related but distinct: every shipped operation currently reports `works: false` under the
  catalog-scoped `credential-not-injected` issue, which describes the `.flux` module path and is
  misleading for the `connector-pack` path that does inject credentials. Worth deciding alongside
  this, though it is a different code.

## Coordinator note at integration (2026-07-31)

Merged at `91ad4f7`, bounced once, rework merged. Both gates green on the integration branch after
the coordinator's full build: zero Rust failures, clippy `-D warnings` clean, `diff` reporting no
drift, web build green and **32/32**.

**The bounce was over `skip_serializing_if`, and the implementor was right to concede it.** Round 1
made `notes` a conditionally-present key, justified as respecting the whole-catalogue artifact fence.
That reading was wrong: `AGENTS.md:129-131` fences an implementor from *regenerating* those
artifacts, not from making them stale, and `:155-171` names the reporting procedure for exactly that
situation. The saving was never required, and what it bought was a permanent hole in the one document
`catalog-json.md` promises a consumer can type once and never test for existence.

**The rework went past the fix, and that is the more valuable half.** The guard that should have
caught this — `site.rs::optional_fields_are_null_rather_than_absent` — enumerated three field names
by hand, so it passed while the guarantee it is named for no longer held. It now renders the document
twice from the same emitter, once with every optional absent and once with each present, and requires
the same key set at the same position. It knows no field by name. The implementor then proved it by
reintroducing the bug and showing the failure (`["issues","works"]` against
`["issues","notes","works"]`) before reverting.

That is the same defect class as [C-151](C-151-hmac-fields-reach-the-manifest.md) and
[C-158](C-158-typescript-catalogue-types-drift.md) — a hand-enumerated list with nothing holding it —
closed by derivation rather than by extending the list.

**Acceptance item 2 was reconciled rather than ticked against words the diff does not implement.** It
asked for a new *`Issue`* code; a `Note` code was built. The review confirmed the contradiction is
real and not the implementor's inference: `works == issues.is_empty()` is documented in
`catalog-json.md`, in `status.rs`'s own doc, and asserted by a pre-existing test, so a fifth issue
code with `works: true` was not expressible. The item's actual intent — no existing token changing
meaning — is met exactly, and `NO_CREDENTIAL` keeps its freshdesk sense.

**One judgement call, decided by the coordinator: the `?? []` fallback in `web/data/catalog.mts`
stays.** The type says `notes: Note[]` required, mirroring the now-unqualified contract, while the
selector tolerates absence. That split is right. The site reads a *committed* catalogue that lags the
emitter for as long as the process mandates — an implementor changes the emitter and must not
regenerate; the coordinator regenerates at integration — and without the fallback `npm run build`
dies during SSR on every operation in that window rather than failing a test. The tolerance is
build-order, not shape, and the comment says so.

**Latent until the first `auth = []` connector ships.** Nothing in the catalogue declares it yet, so
the first provider that does will flip a card from "Not live yet" to "N operations live" and make an
operation pass a `works` filter that has matched nothing since the catalogue existed.
[C-133](C-133-provider-brave-talk-tokens.md) and C-157 are the candidates.

**The adjacent defect is filed, not fixed:** the host repeats this conflation in
`crates/connectors-api/src/api.rs` — see [C-212](C-212-the-host-repeats-the-connected-conflation.md),
which also records a second way the same boolean is wrong, measured against the running service.
