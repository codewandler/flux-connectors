---
id: C-70
title: Ship the Jira connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: basic email+token · tenant URL, like zendesk
---

# Ship the Jira connector

## Goal
Ship the issue tracker most engineering organisations actually run on: issue read, create,
comment and transition, addressed by issue key.

## Acceptance
- [ ] `providers/jira.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://{site}.atlassian.net"`, `vendor = "Jira"`, and a `[[auth]]` entry with `scheme = "basic"`, the account email in `user_env` (`JIRA_USER`) and the API token as the secret (`JIRA_API_TOKEN`), named by `default_auth`.
- [ ] A curated set of roughly five, each with `risk` and `idempotency`, path-addressed:
      issue get, issue create, comment add, comment list, transitions list — over
      `/rest/api/3/issue/{issueIdOrKey}` and its sub-resources.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/jira.flux` and
      `connectors/jira.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **Atlassian Document Format is confronted, not smuggled.** A Jira comment body is ADF — a nested
      rich-text document, not a string. Either declare the nested structure honestly with C-29's `wire`
      paths, or restrict the operation set to fields that are plain and say so. A `description` typed
      `string` that the API rejects is worse than an excluded operation.
- [ ] The tenant template `{site}` is recorded as an unbound base URL, matching how zendesk already
      publishes `unbound-base-url-template` — this is [C-68](C-68-endpoint-binding.md)'s subject, not
      something to invent a binding for here.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- The Basic shape is zendesk's without the `/token` suffix: email in the non-secret half, API token
  as the secret. Getting the halves backwards routes a live credential through the non-secret path,
  which is the security regression C-19's acceptance calls out by name.
- Deliberately excluded pending C-30: JQL search (`/rest/api/3/search`), whose `jql` parameter is the
  most injectable query value in this whole fleet.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
