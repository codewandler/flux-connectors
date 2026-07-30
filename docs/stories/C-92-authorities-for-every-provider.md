---
id: C-92
title: Declare an authority for every shipped provider
pillar: Spec
status: ready
priority: 4
design: docs/designs/global-addressing.md
epic: credential-addressing
areas: [providers, connector-spec]
note: "15 of 16 declare none, so no gid and no credential path renders for them. Its own story because an authority is published under a never-reused contract — this is a decision, not a chore"
---

# Declare an authority for every shipped provider

## Goal
Give every provider the one field that unlocks its addresses. `slack` is the only one with an
`authority`, so it is the only provider with a `gid`, an `oip` or a credential path.

## Acceptance
- [ ] Every `providers/*.toml` declares an `authority`, and a test asserts it — replacing
      `tests/credential_paths.rs::only_providers_with_an_authority_have_credential_paths`, which exists
      to fail here and says so.
- [ ] **Each choice is recorded with its reasoning in the provider file**, because this is the part
      that is not mechanical. `global-addressing.md`'s risk register states the cost plainly:
      *"Choosing an authority commits us. `com.zendesk.api` is our naming of someone else's API. If a
      vendor later publishes their own identifier scheme, ours will not match."*
- [ ] The awkward cases are decided deliberately rather than by pattern-matching, and the file says
      why:
      - **jira** is Atlassian's — `com.atlassian.jira` or `com.atlassian.api`, not `com.jira.api`.
      - **google** already has three services; its authority governs all of them.
      - **babelforce** is a first-party vendor here, so the choice sets a precedent for the others.
      - **openrouter**, **sentry**, **zoom**, **airtable** have no obvious reverse-DNS form.
- [ ] `api_version` where it is knowable, since a `gid` needs both — but note a credential path needs
      only the authority, so the two can land separately if a version is genuinely unclear.
- [ ] The stability contract is restated where it can be seen: an authority, once published, is never
      repointed. `AGENTS.md` already carries the address rule.

## Progress
- Not started. Filed 2026-07-30 with [C-90](C-90-credential-addressing-epic.md), on finding that the
  credential path renders for exactly one provider.

## Notes
- **Deliberately not folded into C-90.** Minting fifteen published identifiers inside a wave about
  secret storage would bury a decision that binds every downstream consumer of an address — the
  catalogue, the manifests, the lockfile, and now credential paths.
- Sequenced after [C-37](C-37-global-addressing.md)'s open question only if that question turns out to
  bear on the authority itself; it does not today, since the ambiguity C-37 records is about tail
  segments *below* the service.
