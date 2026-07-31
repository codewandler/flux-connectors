---
id: C-92
title: Declare an authority for every shipped provider
pillar: Spec
status: in-progress
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
- [x] Every `providers/*.toml` declares an `authority`, and a test asserts it — replacing
      `tests/credential_paths.rs::only_providers_with_an_authority_have_credential_paths`, which exists
      to fail here and says so.
      → `crates/connector-spec/tests/credential_paths.rs::every_shipped_provider_declares_an_authority_and_renders_a_credential_path`
      replaces it and asserts over the whole directory, so a new provider added without one fails on
      the day it is added.
- [x] **Each choice is recorded with its reasoning in the provider file**, because this is the part
      that is not mechanical. `global-addressing.md`'s risk register states the cost plainly:
      *"Choosing an authority commits us. `com.zendesk.api` is our naming of someone else's API. If a
      vendor later publishes their own identifier scheme, ours will not match."*
      → every one of the 37 new declarations carries a wrapped comment block naming the domain it was
      derived from, or the reasoning where it was decided.
- [x] The awkward cases are decided deliberately rather than by pattern-matching, and the file says
      why:
      - **jira** is Atlassian's — `com.atlassian.jira` or `com.atlassian.api`, not `com.jira.api`.
        → `com.atlassian.jira`. The trailing label is the *product* where a single-product vendor
        spells `api`, because Atlassian ships two APIs here and `com.atlassian.api` could only name
        one. `statuspage` follows as `com.atlassian.statuspage`, `microsoft_graph` as
        `com.microsoft.graph`.
      - **google** already has three services; its authority governs all of them.
        → `com.google.api`, one authority; gmail/calendar/drive stay `[[services]]`, which is the
        middle segment of the address rather than three authorities.
      - **babelforce** is a first-party vendor here, so the choice sets a precedent for the others.
        → `com.babelforce.api`, derived exactly as everybody else's and given no special spelling for
        being ours.
      - **openrouter**, **sentry**, **zoom**, **airtable** have no obvious reverse-DNS form.
        → three of the four do, and the file says where it came from: `ai.openrouter.api` (the
        `io.fly.api` precedent — a non-`com` TLD is just the first label), `io.sentry.api` (Sentry's
        own Maven group is `io.sentry`), `us.zoom.api` (Zoom's own Android packages are `us.zoom.*`).
        `airtable` was not in fact awkward: `com.airtable.api` from `airtable.com`.
- [x] `api_version` where it is knowable, since a `gid` needs both — but note a credential path needs
      only the authority, so the two can land separately if a version is genuinely unclear.
      → declared for the 12 providers whose own file already spells the vendor's version, taken from
      `base_url` (`clickup`, `cloudflare`, `docusign`, `freshdesk`, `gitlab`, `launchdarkly`, `miro`,
      `okta`, `statuspage`, `twilio`, `webflow`) or from every operation path (`zendesk`). The other
      25 are left without one on purpose — see Progress.
- [x] The stability contract is restated where it can be seen: an authority, once published, is never
      repointed. `AGENTS.md` already carries the address rule.
      → each of the 37 blocks closes by quoting it.

## Progress
- **Done.** The `note` in this file's frontmatter is stale and was left alone so the generated board
  does not desync: the real figure at the merge base was **37 of 44** providers declaring no
  authority, not 15 of 16. All 44 declare one now, and `web/public/catalog.json` carries them.
- **The Basic branch of `auth::acquire` now has a shipped consumer, which was the sharper half of this
  story.** All three `BasicJoin` connectors — zendesk, jira, twilio — declared `authority: None`, so
  `Credentials::reference` refused with `NoCredentialAddress` before the configuration port was ever
  consulted and the whole mechanism was unreachable in anger. C-198's two tests consequently lived in
  `crates/connector-pack/src/credentials.rs` over a `Box::leak`ed zendesk doctored with an authority.
  Both have moved to `crates/connector-pack/tests/credentials.rs` and now drive the real shipped
  catalogue entry through the public `Operation::build_authenticated_request`; nothing is doctored.
  C-198's deliberately inverted `a_basic_connector_refuses_because_it_has_no_credential_address` is
  removed rather than relaxed — it pinned a wall that no longer exists.
- **Why 25 providers have no `api_version`.** Only a version the connector's own file already spells
  was taken. Anything else would mean asserting a vendor fact from memory, and an `api_version` is
  published under the same never-reused contract as the authority — so the acceptance's own escape
  hatch applies: a credential path needs the authority alone, and the two can land separately. Those
  25 render a `pid` and a credential path today and gain a `gid` when someone checks the vendor's
  docs.
- **`the_three_outcomes_are_distinguishable` no longer sources its `Ok(None)` arm from `providers/`.**
  It used `shipped("zendesk")`, which only worked while zendesk had no authority. `authority` is still
  `Option` and a host still has to tell "this connector has no address" apart from "you asked for a
  credential it does not declare", so the case is now built from a small in-test fixture.
- Filed 2026-07-30 with [C-90](C-90-credential-addressing-epic.md), on finding that the credential
  path renders for exactly one provider.

## Notes
- **Deliberately not folded into C-90.** Minting fifteen published identifiers inside a wave about
  secret storage would bury a decision that binds every downstream consumer of an address — the
  catalogue, the manifests, the lockfile, and now credential paths.
- Sequenced after [C-37](C-37-global-addressing.md)'s open question only if that question turns out to
  bear on the authority itself; it does not today, since the ambiguity C-37 records is about tail
  segments *below* the service.
