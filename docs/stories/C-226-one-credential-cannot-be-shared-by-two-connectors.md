---
id: C-226
title: "One vendor credential cannot be shared by two connectors, so an operator pastes the same Atlassian token twice"
pillar: Bridge
status: ready
priority: 2
design: docs/designs/auth-seam.md
epic: connectors-api
areas: [connector-spec, bridge, host]
note: "the answer C-219's probe returned on 2026-07-31, asserted rather than described. Verified by the coordinator at crates/connector-spec/src/ir.rs:1166 — credential_ref_for keys on (tenant, authority, service, leaf), and C-92 derives an authority per product, so jira and confluence resolve to different paths for one token"
---

# One vendor credential cannot be shared by two connectors

## Goal

Let two connectors that genuinely share one vendor credential resolve to one stored value, so an
operator supplies it once.

## What was measured

[C-219](C-219-provider-confluence.md) shipped Confluence specifically to answer this, and asserted
the answer in its contract test rather than describing it — loading **both** shipped provider files
and rendering both addresses:

```
tenants/<tenant>/com.atlassian.jira/api_token
tenants/<tenant>/com.atlassian.confluence/api_token
```

Same tenant, same leaf. **The authority segment is the entire difference.** Jira and Confluence use
the same host, the same account and the same API token, and the operator must paste it twice.

The mechanism, confirmed independently at `crates/connector-spec/src/ir.rs:1166`:
`credential_ref_for` builds a `CredentialRef` from `(tenant, authority, DEFAULT_SERVICE, leaf)`, and
[C-92](C-92-authority-per-product.md) derives an authority per *product*. Nothing is broken; the
address is doing exactly what it says.

## The precise finding

[C-90](C-90-credential-addressing-epic.md)'s premise is that *an address is a place, not a
per-connector copy*. That holds **within** a connector — `postmark`'s two tokens prove it, and
`credential_ref_for_elides_the_service_and_the_two_tokens_still_never_collide` pins it. It was never
wired to reach **across** two connectors, because until Confluence landed nothing in the catalogue
was close enough to expose the difference.

This is a gap in reach, not a contradiction. Stating it that way matters, because the tempting fix —
collapsing the authority — is the wrong one.

## The rejected branch, measured rather than asserted

C-219 considered shipping one `atlassian` connector with jira and confluence as *services*, which
**would** have delivered single-paste sharing at `tenants/<t>/com.atlassian/api_token`. It refused,
for a reason worth preserving: `com.atlassian.jira` is **published**. Repointing it moves every Jira
gid and every already-provisioned tenant credential path — a migration with a deprecation window,
not a refactor.

So the layout decision was right and this story is the clean fix: make sharing declarable **without
repointing anything already published**.

## Acceptance

- [ ] **Failing-first test:** two connectors declaring a shared credential resolve to one address,
      and an operator who supplied it for the first is not asked again by the second. It fails
      today — C-219's `the_two_atlassian_connectors_do_not_share_a_credential_address` is the
      assertion that currently pins the opposite, and it must be updated rather than deleted.
- [ ] The sharing has a **direction**: one side owns the value and the other aliases it. A symmetric
      declaration leaves "which one is authoritative" unanswered, which surfaces the first time the
      two disagree about scopes.
- [ ] A rule for what happens when the two connectors declare **different** requirements for the same
      credential — different scopes, different placements. Refusing to load is an acceptable answer;
      silently taking one is not.
- [ ] A migration path for tenants that already stored the value twice. Both copies exist today and
      one of them becomes the alias; say which and what happens to the other.
- [ ] **Authority uniqueness is enforced.** C-219 found that nothing in the repository stops a future
      provider file from silently claiming `com.atlassian.jira`. That is a cheap, checkable
      invariant and it is a precondition for trusting an alias to point somewhere specific.
- [ ] `crates/connectors-api/src/index.html` shows the operator that a connector's credential is
      already satisfied by another connection, rather than presenting an empty box that happens to
      work. Sharing the operator cannot see is sharing they will not trust.

## Notes

- **Do not fix this by collapsing the authority.** `com.atlassian.jira` is published; see the
  rejected branch above. If a service split is ever taken, it is its own story with a deprecation
  window, and this one should land first regardless.
- Sequencing: this sits between `connector-spec` (the address) and `connectors-api` (the store and
  the UI). The address half can land alone; the UI half depends on
  [C-212](C-212-the-host-repeats-the-connected-conflation.md) having settled how a connector's
  readiness is expressed, since "satisfied by another connection" is a fourth state.
- Read `docs/designs/auth-seam.md` §7.5 first. C-221 cites it refusing pre-composed credentials —
  two names for one value — and an alias must not become that by another route.
