---
id: C-70
title: Ship the Jira connector
pillar: Spec
status: done
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
- [x] `providers/jira.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [x] `base_url = "https://{site}.atlassian.net"`, `vendor = "Jira"`, and a `[[auth]]` entry with `scheme = "basic"`, the account email in `user_env` (`JIRA_USER`) and the API token as the secret (`JIRA_API_TOKEN`), named by `default_auth`.
- [x] A curated set of roughly five, each with `risk` and `idempotency`, path-addressed:
      issue get, issue create, comment add, comment list, transitions list — over
      `/rest/api/3/issue/{issueIdOrKey}` and its sub-resources.
      **Six, and on `/rest/api/2/`** — the version change is the ADF decision below; the sixth is
      `jira-issue-transition`, without which the Goal's "transition" is unreachable (listing the
      available transitions does not perform one). See Progress.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [x] `cargo run -p connector-cli -- build` emits `connectors/jira.flux` and
      `connectors/jira.connector.toml`, both committed, and a second build is byte-identical.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [x] **Atlassian Document Format is confronted, not smuggled.** A Jira comment body is ADF — a nested
      rich-text document, not a string. Either declare the nested structure honestly with C-29's `wire`
      paths, or restrict the operation set to fields that are plain and say so. A `description` typed
      `string` that the API rejects is worse than an excluded operation.
- [x] The tenant template `{site}` is recorded as an unbound base URL, matching how zendesk already
      publishes `unbound-base-url-template` — this is [C-68](C-68-endpoint-binding.md)'s subject, not
      something to invent a binding for here.

## Progress
- **Done.** Six operations on the v2 issue resource tree, 68 artifacts across 7 providers, full gate
  green plus the `web/` build and tests.
- **ADF: confronted by choosing the API version whose fields are plain, not by typing a lie.**
  C-29's `wire` field is a dot-separated path assembled by `connector-flux`'s `BodyNode`, which is
  `Leaf` or `Branch(BTreeMap)` and **has no array variant** (`crates/connector-flux/src/op.rs`). An
  ADF document's `content` is an *array* of block nodes, so there is no `wire` spelling that places
  the caller's text inside it — the nesting is inexpressible, not merely awkward. Of the three
  remaining options, two are refused by this story (a v3 `body` typed `string`, which Jira rejects
  with `400 Comment body is not valid!`; a free-form `body_schema` a model must invent ADF into), so
  the file addresses `/rest/api/2/`, where Atlassian documents the *same collection of operations*
  with a plain-string comment body. The cost is a text format — comment and description text is wiki
  markup rather than rich content — and it is stated in the header comment, in the operation
  descriptions a model reads, and pinned by
  `crates/connector-flux/tests/jira_connector.rs::every_jira_operation_is_addressed_by_issue_key_on_the_v2_issue_resource`
  so a later bulk upgrade to v3 fails loudly. Closing it properly needs an array-capable body path
  (a `wire` spelling for an array element plus a `BodyNode::List`) under C-29.
- **Left out for C-56** (optional body field ⇒ explicit `null`): `fields.description` on issue create
  — so a created issue carries a summary and no description, and it was deliberately *not* promoted
  to `required` because Jira does not require it and publishing a contract the vendor lacks is the
  other half of the same dishonesty; `fields.priority`, `fields.labels`, `fields.assignee`,
  `fields.parent`, `fields.duedate`; and **`visibility` on comment add**, whose absence means every
  comment this connector posts is visible to everyone who can see the issue, including customers on
  a service-management portal. That is the documented vendor default and it is stated in the
  operation's own description, so a model is not misled — but an internal-note capability needs C-56.
- **Issue update (`PUT /rest/api/2/issue/{issueIdOrKey}`) is excluded**, and this is the sharpest
  C-56 consequence in the fleet so far: an update names only the fields it changes, so under C-56 the
  untouched fields would travel as explicit nulls — and on Jira a null *clears* a field rather than
  being ignored. That is a data-loss write, so the operation is not shipped. Workflow state is still
  reachable through the two transition operations.
- Six rather than five: `jira-issue-transitions-list` only *reports* the available transitions, so
  `jira-issue-transition` is what makes the Goal's "transition" real. The curated count is claimed as
  `("jira", 6)` in `connector-spec`'s `operation_selection_stays_curated`.

## Notes
- The Basic shape is zendesk's without the `/token` suffix: email in the non-secret half, API token
  as the secret. Getting the halves backwards routes a live credential through the non-secret path,
  which is the security regression C-19's acceptance calls out by name.
- Deliberately excluded pending C-30: JQL search (`/rest/api/3/search`), whose `jql` parameter is the
  most injectable query value in this whole fleet.
- Also excluded pending C-30, all of them query values: `fields` and `expand` on issue get (so it
  returns Jira's entire default field set, custom fields included), and `startAt`/`maxResults` on
  comment list (so it returns the first default-sized page only, truncating a long discussion).
  `Quirks.pagination` cannot rescue the latter — its `page_param`/`size_param` are defined as *query*
  parameters, so declaring the quirk means declaring the parameters.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
