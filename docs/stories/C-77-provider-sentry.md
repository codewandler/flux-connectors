---
id: C-77
title: Ship the Sentry connector
pillar: Spec
status: done
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · trailing slashes are load-bearing
---

# Ship the Sentry connector

## Goal
Ship error tracking, so an agent can read and triage an issue it was told about instead of
being handed a URL.

## Acceptance
- [x] `providers/sentry.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [x] `base_url = "https://sentry.io"`, `vendor = "Sentry"`, and a `[[auth]]` entry with `scheme = "bearer"` over `SENTRY_AUTH_TOKEN`, named by `default_auth`.
- [x] A curated set of roughly four over `/api/0`: issue get, issue update (resolve/ignore),
      project get, issue events latest — path-addressed by organization, project and issue id.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [x] `cargo run -p connector-cli -- build` emits `connectors/sentry.flux` and
      `connectors/sentry.connector.toml`, both committed, and a second build is byte-identical.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [x] **The trailing slash is preserved exactly.** Sentry's paths end in `/`, and dropping it
      redirects or 404s depending on the endpoint. A test pins the emitted URL for every operation, since
      this is the kind of detail a later "tidy up" silently breaks.
- [x] Issue update is a write that changes triage state a team relies on; its `risk` says so and it is
      not idempotent unless the vendor documents the status transition as such.

## Progress
- **Done.** `providers/sentry.toml` ships 4 operations over `/api/0`; artifacts built and committed
  (`107 artifacts up to date (12 providers checked)`, second build byte-identical).
- The trailing-slash gate is `crates/connector-flux/tests/sentry_connector.rs`
  `the_emitted_url_of_every_operation_is_pinned_including_its_trailing_slash`, which pins the whole
  `$url = fmt(...)` line of each of the four operations rather than asserting `ends_with('/')` — the
  property form is satisfied by `.../issues/`, which is the issue *list*.
- Two hand-maintained files carry the one line each this provider needs: the `mod`/`PROVIDERS` entry
  in `crates/catalog/src/generated.rs`, and `("sentry", 4)` in `connector-spec`'s
  `operation_selection_stays_curated`. No new provider list was introduced.
- **What a resuming agent should look at:** when C-56 lands, `assignedTo` becomes declarable on
  `sentry-issue-update` — read that operation's comment first, because a `null` on that field is
  *applied* by Sentry and unassigns the issue. When C-30 lands, the issues list plus
  `statsPeriod`/`environment`/`project` become expressible.

## Notes
- Self-hosted Sentry has a different host, which is [C-68](C-68-endpoint-binding.md)'s subject; this
  story targets `sentry.io` and records the limitation rather than inventing a binding.
- Deliberately excluded pending C-30: the issues *list* with its `query` parameter, which is Sentry's
  own search syntax and therefore the most injectable value it exposes.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
- Deliberately excluded pending C-56 (an omitted optional body field travels as an explicit `null`,
  and on Sentry's partial update a `null` is *applied* rather than rejected): `assignedTo` — the field
  that reassigns an issue, and the one this gap bites hardest on, because a `null` destroys the
  existing assignment — plus `statusDetails` and with it the `resolvedInNextRelease` / `inRelease` /
  `ignoreDuration` / `ignoreCount` transitions, and the `hasSeen`, `isBookmarked`, `isSubscribed`,
  `isPublic` and `merge` flags. `sentry-issue-update` therefore sets `status` and nothing else.
- Also excluded pending C-30, beyond the issues list: the `statsPeriod` / `environment` / `project`
  filter triple every organization-scoped endpoint accepts, and `cursor` paging — which is why the
  issue's *latest* event is the operation rather than its event list.
- `sentry-issue-update` is `risk = "high"`: Sentry's unresolved queue is the team's inbox and its
  alerting is driven by issue status, so `ignored` silences a live error for the whole organization.
  It is `non_idempotent` because Sentry documents no idempotency guarantee for the transition.
- No `response_schema` is declared, unlike asana's: Sentry returns the resource with no envelope, so
  there is no shape a consumer could not read off the vendor reference.
