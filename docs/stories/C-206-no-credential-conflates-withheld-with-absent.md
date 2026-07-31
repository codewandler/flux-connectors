---
id: C-206
title: "`no-credential` tells consumers a public endpoint is disabled for their protection"
pillar: Surfaces
status: in-progress
priority: 2
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
- [x] A **new** issue code is added rather than `NO_CREDENTIAL` changing meaning, per the stability
      rule at `status.rs:64-70`. `NO_CREDENTIAL` keeps its freshdesk sense exactly.
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
- **Zero drift by construction.** `notes` is `skip_serializing_if = "Vec::is_empty"`, so
  `web/public/catalog.json` is byte-identical and no whole-catalogue artifact needed regenerating.
  `a_status_with_no_note_encodes_exactly_as_it_did_before` holds that. It is the one key in
  `catalog-json.md` that is not always present, documented as such.

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
