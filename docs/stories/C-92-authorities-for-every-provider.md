---
id: C-92
title: Declare an authority for every shipped provider
pillar: Spec
status: done
priority: 4
design: docs/designs/global-addressing.md
epic: credential-addressing
areas: [providers, connector-spec]
note: "MEASURED 37 of 44 declared none (the original '15 of 16' was stale by two fleet waves); so no gid and no credential path renders for them. Its own story because an authority is published under a never-reused contract — this is a decision, not a chore"
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
        spells `api` — but the predicate is **the product's own API domain, not the parent's product
        count**, which is the correction the review forced. Jira has no live `jira.com` API surface
        (the host is `{site}.atlassian.net`), so Atlassian's domain is the only one available.
        `microsoft_graph` is the same branch (`com.microsoft.graph`; there is no `graph.com`).
        `statuspage` is **not**: it kept `statuspage.io` and is now `io.statuspage.api`.
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
      → **30 of 44 declare one.** The first pass declared 12 and the review found the rule had been
      applied to only half its own scope: it took a version from `base_url` but not from the
      operation paths, even though `zendesk` had qualified that way. 17 more were added
      (`airtable v0`, `anthropic v1`, `asana 1.0`, `box 2.0`, `dropbox 2`, `figma v1`, `hubspot v3`,
      `jira 2`, `notion v1`, `openai v1`, `openrouter v1`, `sendgrid v3`, `sentry 0`,
      `shopify 2024-10`, `stripe v1`, `zoom v2`, `babelforce v2`). The remaining 14 are each
      explained in their own file: 3 version per-operation and so have no connector-level truth
      (`datadog` v1+v2, `vercel` v7–v13, `salesforce` v59.0 plus an unversioned OAuth path), 8 carry
      no version segment at all, and 3 (`fly`, `google`, `microsoft_graph`) declare it per service,
      which is C-49's mechanism for exactly this.
- [x] The stability contract is restated where it can be seen: an authority, once published, is never
      repointed. `AGENTS.md` already carries the address rule.
      → every authority block closes by quoting it.

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
  gone, and an equivalent assertion has replaced it (see the rework note below).
- **Why 14 providers have no connector-level `api_version`.** Each says so in its own file, because
  an unexplained omission is indistinguishable from an overlooked one — which is how the first pass
  lost 17 of them. Three version per operation and have no connector-level truth to state; eight
  carry no version segment anywhere (GitHub and Intercom version by request header instead); three
  declare it per service.
- **`the_three_outcomes_are_distinguishable` no longer sources its `Ok(None)` arm from `providers/`.**
  It used `shipped("zendesk")`, which only worked while zendesk had no authority. `authority` is still
  `Option` and a host still has to tell "this connector has no address" apart from "you asked for a
  credential it does not declare", so the case is now built from a small in-test fixture.
### Rework after review (C-92-v2)

The first pass shipped and was reviewed REWORK on two substantive findings. Nothing had reached
crates.io, so no address was cemented and all of it was free to correct.

- **The multi-product rule had the wrong predicate, and I had it pointing in two directions at
  once.** I keyed on *"does the vendor own several products"* — a fact about the parent — and so put
  Statuspage under Atlassian while keeping SendGrid out from under Twilio, two structurally identical
  cases. Worse, the reason recorded for both was the same false claim: that a shared vendor label
  would make two products "share a credential directory". **It would not.** An authority is a single
  path segment (`tenants/<tenant>/<authority>/<service>/<credential>`, `AGENTS.md`) handed whole to
  `CredentialRef::new`, so `com.twilio.sendgrid` and `com.twilio.api` are siblings sharing nothing.
  Credential storage does not bear on the choice in either direction, and I had used it as a reason
  both *for* and *against* the same shape.
- **The corrected rule keys on the product, not the parent: does this product still publish its own
  API domain?** If it does, it keeps its own authority; if it does not, it takes the parent's domain
  with the product as the trailing label. That is a checkable fact rather than a judgement, and it
  decides every case in the catalogue consistently — including the three acquired-product pairs,
  which is the part I had not seen.
- **The tree had already decided it and I missed the evidence.** Salesforce owns Slack, and
  `providers/slack.toml` is `com.slack.api`, not `com.salesforce.slack` — one of only two
  authorities present at the `v0.5.0` tag (`io.fly.api` is the other), so it is the one spelling here
  that genuinely cannot change. Any rule that put acquired products under their parent would have
  contradicted the project's own published set on day one.
  → **`com.atlassian.statuspage` → `io.statuspage.api`.** Statuspage kept `statuspage.io`, and
  `api.statuspage.io` is its host. It was the only authority in the catalogue that rejected the
  product's own live API domain in favour of the parent.
  → **`com.sendgrid.api` kept, reasoning replaced.** The value was right for a reason I had not
  given.
  → **`com.atlassian.jira` and `com.microsoft.graph` unchanged**, and now resting on the checkable
  fact rather than on the parent's product count: neither Jira nor Graph publishes a domain of its
  own.
- **`com.anthropic` → `com.anthropic.api`.** It was the one authority fitting neither branch of the
  rule — not `<vendor>.api`, not `<vendor>.<product>` — and I had wrongly treated it as immutable.
  It is not published: it landed after the `v0.5.0` tag and `git show v0.5.0:providers/anthropic.toml`
  does not resolve, because the file did not exist at the only release cut so far. Its own comment
  advertised a `gid` of `com.anthropic/models:2023-06-01` that never rendered, since no `api_version`
  was declared; it now has `v1` from its paths, and the file explains why the dated
  `anthropic-version` header is a second axis carried as a `const_headers` entry rather than the
  address's version.
- **The fail-closed assertion is restored**, as
  `crates/connector-pack/tests/credentials.rs::a_provider_without_an_authority_is_refused_rather_than_addressed`.
  Removing it was wrong: `Provider::authority` is still `Option`, the loader still accepts a TOML
  that omits it, and `grep -rn NoCredentialAddress crates/` had been left finding only doc comments
  and message text — nothing asserted the refusal anywhere. That no *shipped* provider reaches the
  branch is what makes the test necessary, not what makes it redundant, and it is the same call
  already made one crate over for `the_three_outcomes_are_distinguishable`. Verified non-vacuous by
  mutation: replacing the refusal with a fallback authority makes it fail.

- Filed 2026-07-30 with [C-90](C-90-credential-addressing-epic.md), on finding that the credential
  path renders for exactly one provider.

## Notes
- **Deliberately not folded into C-90.** Minting fifteen published identifiers inside a wave about
  secret storage would bury a decision that binds every downstream consumer of an address — the
  catalogue, the manifests, the lockfile, and now credential paths.
- Sequenced after [C-37](C-37-global-addressing.md)'s open question only if that question turns out to
  bear on the authority itself; it does not today, since the ambiguity C-37 records is about tail
  segments *below* the service.
