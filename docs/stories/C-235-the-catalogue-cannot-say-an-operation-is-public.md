---
id: C-235
title: "The embedded catalogue emits `[]` for both a withheld and a positively-public operation, so no host reading it can tell them apart — and that is now a user-facing surface"
pillar: Bridge
status: done
design:
epic: connectors-api
areas: [codegen, bridge, host]
note: "filed by C-206 as catalogue-side and low-stakes; raised to priority 1 by C-212 on 2026-07-31, which needed the distinction to serve an operator-facing state and could not get it. Verified at connector-cli/src/catalog.rs:356-365 — credential_mechanisms maps effective_auth, which is empty in both cases"
---

# The catalogue cannot say an operation is public

## Goal

Let the embedded catalogue carry the distinction [C-206](C-206-no-credential-conflates-withheld-with-absent.md)
established, so a host reading it can tell "this vendor needs no credential" from "this operation
withholds what it needs".

## What was measured

`crates/connector-cli/src/catalog.rs:356-365`:

```rust
pub(crate) fn credential_mechanisms<'a>(connector: &'a Connector, operation: &'a Operation)
    -> Vec<Vec<&'a str>> {
    connector.effective_auth(operation).iter()
        .map(|mechanism| mechanism.iter().map(String::as_str).collect())
        .collect()
}
```

`effective_auth` returns an empty list in **both** cases, so `catalog::Operation::credentials` is
`[]` for a positively-public operation and `[]` for a withheld one. C-206 taught the *published*
`catalog.json` to distinguish them through `Operation::auth` — `None` (inherits) versus `Some([])`
(explicitly none) — but the **embedded** catalogue, which is what a Rust host links against, carries
only the flattened mechanism list.

## Why it moved from low-stakes to priority 1

C-206 filed this as a catalogue-side gap affecting `catalog.json`, whose audience is a machine. That
was right at the time. [C-212](C-212-the-host-repeats-the-connected-conflation.md) has since made it
**user-facing**: the host now serves a three-state `wiring` field so an operator scanning 53
connectors can see which ones are ready. That state is computed from the embedded catalogue, which
cannot make the distinction — so the host infers it from an absence.

The consequence, judged and accepted deliberately at C-212's integration rather than discovered
later: **`freshdesk` is served as `no-credential-required`, which is the right state for an imprecise
reason.** Freshdesk deliberately declares no `[[auth]]` at all (`providers/freshdesk.toml:56-68`)
because its API key belongs in the Basic *username* position, and `AuthMethod::user_env` is
documented as config rather than a gated secret — so declaring it there would remove it from the
secret gate. There genuinely is nothing for an operator to supply through this host, so the state is
operationally correct; it simply is not reached by C-206's positive declaration. `Wiring`'s doc
comment says so where a reader meets it.

The alternative was worse and is why this was not bounced: rendering `not-wired` would send an
operator hunting a token this repository refuses to hold — the exact defect C-212 exists to remove.

## Acceptance

- [x] **Failing-first test:** the embedded catalogue distinguishes a positively-public operation from
      a withheld one. It cannot today. Name it.
      → `crates/connector-cli/tests/credential_requirement.rs::the_embedded_catalogue_tells_a_public_operation_from_a_withheld_one`.
      At the base both render `credentials: &[],` and the assertion fails on two equal lists.
- [x] The distinction uses **C-206's vocabulary**, not a third spelling. The published catalogue
      already carries `no-credential-required`; the embedded one restating that in different words is
      how two surfaces drift — the defect C-212 was careful to avoid and this story must not
      reintroduce.
      → `catalog::CredentialRequirement::as_str` returns `no-credential-required` and
      `no-credential`, the two codes `docs/designs/catalog-json.md` already publishes;
      `crates/catalog/tests/embedded_operations.rs::the_requirement_tokens_are_the_published_ones`
      pins them. Stronger than agreeing spellings: `connector_cli::status::credential_requirement`
      is the **one** derivation and both backends render its answer, pinned by
      `catalog.rs::tests::the_table_and_the_published_status_read_the_same_classification`.
- [x] `connectors-api` reads the real distinction instead of inferring it from an empty list, and
      `Wiring`'s doc comment stops explaining an approximation.
      → `api.rs::wiring_of` and `is_callable` read `catalog::Operation::credential_requirement`;
      the "what the host can prove, and what it inherits" paragraph is replaced by what it now
      reads. `tests/wiring.rs::a_withheld_credential_is_not_served_as_a_vendor_that_needs_none`.
- [x] **Decide what `freshdesk` should be**, now that a fourth answer is available: a connector whose
      credential this repository deliberately does not model. It may still be
      `no-credential-required`, but that becomes a decision with a reason rather than a coincidence.
      Record it either way.
      → **`no-credential`**, a fourth `Wiring` variant. Recorded in `providers/freshdesk.toml`
      (§"WHAT THIS CONNECTOR IS, AS A STATE"), in `Wiring::NoCredential`'s doc, and in Progress
      below.
- [x] Nothing in the shipped catalogue changes shape without the whole-catalogue artifacts being
      regenerated by the coordinator — this touches `crates/catalog/src/generated.rs` and
      `web/public/catalog.json`, both fenced.
      → Neither moved: `generated.rs` names providers only, and `catalog.json` is unchanged because
      `site.rs` and `status::of`'s output are unchanged. `git status` shows both clean.
      **`connectors.lock` did move** and is committed — see Progress.

## Progress

**2026-08-01 — implemented on `impl/C-235`.** The embedded catalogue carries
`catalog::Operation::credential_requirement`, a three-state enum beside the mechanism list.
`937 artifacts up to date (53 providers checked)` after the full build; the count is unchanged and
53 per-provider tables plus `connectors.lock` moved.

- **A typed column was the answer, and the regeneration was the price.** C-413 declined it for
  `expose` because its Acceptance required no shipped artifact to move; this story's Acceptance says
  the opposite in item 5, and there was no alternative carrier anyway — the emitted Flux states
  `expose` but says nothing about auth (`connector-flux` emits none at all until C-10), so there was
  no existing line for the fact to ride on. The two axes stay separate: exposure is *does a model
  see it*, this is *what does it need to authenticate*, and an operation can be any combination.
- **One derivation, two renderings.** `connector_cli::status::credential_requirement` is the single
  classifier; `status::of` turns it into `catalog.json`'s issue-or-note and
  `catalog.rs::credential_requirement` maps it into the embedded table through an exhaustive match
  (the same shape `risk()` and `runtime()` use). A fourth state is a compile error in both places
  rather than a silent divergence — which is what C-206 and C-212 each had to fix one surface at a
  time.
- **Freshdesk is `no-credential`, and it is a decision.** A fourth `Wiring` variant, in C-206's own
  token rather than a new spelling of it: `NO_CREDENTIAL` is extended to a second surface with its
  existing sense, not renamed. Two things follow that the old answer got wrong. Its nine operations
  are now `callable: false` — an unauthenticated request to an endpoint that wants a credential is a
  401, and `is_callable` used to read an empty mechanism list as *needs nothing, so anyone can call
  it*. And the page says "nothing to supply, and nothing to do" rather than "nothing to supply",
  which an operator reasonably read as *ready*.
- **What a consumer must do.** Nothing, unless it switches on the host's `wiring` field: no code in
  `catalog.json` changed meaning, none was renamed, and the document is byte-identical. A consumer
  enumerating `wiring` states must add `no-credential`, because freshdesk moved onto it. Recorded in
  `docs/designs/catalog-json.md` §"The same two words travel into the embedded catalogue".
- **C-206's `status.rs` half was already closed**, and the dispatch's premise that it was not is
  stale. `status.rs:128-131` today is `Issue`'s doc comment; the comment that *stated the
  distinction without making it* was quoted from the pre-C-206 file. The current
  `status::of` gates the freshdesk wording behind `declares_no_credential_is_needed`, so the
  published sentence has not been wrong for a public endpoint since C-206 landed. This story
  therefore closes the **host** half, which was the one still saying the wrong thing to a person.
- **The genuinely-public state is still latent.** Nothing ships `auth = []`, so it is proved against
  fixtures — in the emitter (`catalog.rs`), through the real loader
  (`tests/credential_requirement.rs`), and in `api.rs`'s `wiring_of` unit tests. The half that
  *is* assertable over shipped data — freshdesk — is asserted over the real HTTP surface.
- **`connectors.lock` is committed.** It is whole-catalogue and therefore coordinator-owned, but the
  dispatched gate ends in `build` + `diff` and a stale lock makes `diff` exit non-zero; this story
  runs solo by its own Notes. Regenerate it at integration like any other generated conflict.

## Notes

- Sequencing: this changes the emitter and therefore every generated artifact, so it runs solo or
  first in a wave. It collides with every provider story by definition.
- Read C-206 first, then C-212's `Wiring` enum. The two together define the vocabulary; this story
  only has to make the embedded catalogue able to express it.
- The latent cases are still latent: nothing in the shipped catalogue declares `auth = []` yet, so a
  genuinely-public connector ([C-133](C-133-provider-brave-talk-tokens.md), C-157) would be the first
  real instance. Fixing this before one arrives is the cheap ordering — the alternative is
  discovering it through a connector that looks broken.
