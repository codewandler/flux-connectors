---
id: C-22
title: Auth conformance matrix across provider archetypes
pillar: Bridge
status: backlog
design: docs/designs/unified-auth.md
epic: unified-auth
areas: [connector-spec]
note: a new provider shape must fail at the model, not at request time
---

# Auth conformance matrix across provider archetypes

> **Amendment ([C-86](C-86-connector-configuration-epic.md)).** **Landed as
> `crates/connector-spec/tests/auth_archetypes.rs`**, with the question sharpened: it is not enough
> that the model can *express* an archetype, it must be able to say **what form that archetype
> generates**. Each case names the real provider it is drawn from, as this story required.
>
> Covered: prefixed header (slack), basic join with a vendor marker (zendesk) *and* without one (jira,
> which proves the difference is declared rather than assumed), raw-value header (shopify), no
> credential at all (freshdesk), AND-sets and OR-alternatives (babelforce), and the signing secret.
>
> **The explicit failing case this story asked for is OAuth2**, not `hmac` as predicted:
> `no_shipped_provider_exercises_oauth_yet` asserts that `OAuth2Spec` is a landed type no provider
> uses, and its failure message says what to replace it with. [C-88](C-88-prove-oauth2.md) closes it.
> `hmac` turned out to be expressible after all — as `AuthScheme::Signing` plus a channel binding's
> verification block — though only in the inbound direction; SigV4 remains open.

## Goal
Pin the unified model against one case per real-world archetype, so an unsupported credential shape
fails loudly in a test rather than silently at the first live request.

## Acceptance
- [ ] One conformance case per archetype, each asserting the model expresses it **and** what the
      emitted placement would be:
      - raw-value header (`x-api-key`, `PRIVATE-TOKEN`)
      - prefixed header (`Authorization: Bearer`, `Authorization: Token`)
      - basic join (zendesk `<email>/token`, freshdesk `api_key`+`X`)
      - query-parameter key
      - two credentials sent together (AND)
      - alternatives (OR), asserting the documented selection rule
      - unauthenticated operation (explicit empty set)
      - OAuth2 client-credentials and password grants
      - locally-signed JWT
- [ ] Each case names the real provider it is drawn from, so the matrix documents reality rather than
      hypotheticals.
- [ ] An archetype the model **cannot** express is an explicit, documented failing case rather than a
      gap discovered later — `hmac`/SigV4 is the expected one.
- [ ] The matrix runs in CI.

## Progress
- (not started)

## Notes
- This is the story that makes "unified" a claim with evidence behind it instead of an aspiration.
- Depends on C-19; best written alongside it so the model is designed against the matrix rather than
  the matrix retrofitted to the model.
