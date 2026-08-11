---
id: C-527
title: "GitHub and GitLab can answer what a token reaches"
pillar: Connector
status: done
priority: 0
areas: [providers, connector-flux, connector-spec, tests]
note: "Every forge operation took {owner}/{repo} or {project_id} as given, so a caller holding only a token could not find an organisation, a group or a repository — the connector described a repository nobody could name"
---

# GitHub and GitLab can answer what a token reaches

## Goal

Give the two forge connectors the entry-point reads a caller needs before any other operation in
them is usable, and give GitHub the `verify` read it never had.

Measured before starting: GitHub declared 9 operations and **no `verify`**; GitLab declared 7. Every
GitHub operation took `{owner}` and `{repo}` as given, and every project-scoped GitLab operation took
a numeric `{project_id}` — which `providers/gitlab.toml`'s own header comment admits is the id
"nobody carries in their head". Nothing in either connector produced those values, so a host holding
a valid token could not enumerate a single thing it could reach.

## Acceptance

- [x] GitHub gains `github-user-get`, `github-org-list`, `github-org-repo-list` and
      `github-user-repo-list`, each as one exact `[[patch.operations]]` selection from the vendored
      first-party OpenAPI document. `patch.select` stays empty — this connector opts in one
      `operationId` at a time.
- [x] GitHub declares `verify = "github-user-get"`. It had none, so a host reading its manifest had
      no Test connection read to call while 42 other connectors did.
- [x] GitLab gains `gitlab-group-list` and `gitlab-project-list`, hand-authored. `gitlab-project-list`
      returns the numeric `id` every other GitLab operation requires, and `http_url_to_repo` beside
      it — the HTTPS clone address, declared because cloning is **not** a connector operation and a
      caller needs the address from somewhere.
- [x] The new reads carry the vendor's scalar filters, including free-text `search` on both GitLab
      reads. The four frozen GitHub collection reads and the three published GitLab reads keep their
      exact parameter sets.
- [x] Every generated artifact regenerated; `diff` reports **1108 artifacts up to date (55 providers
      checked)**, up from 1102, with 835 operations. The five originally published GitHub operations
      keep their Flux bytes — `the_five_published_operations_keep_their_flux_bytes` is unchanged and
      passes.

## Progress

- 2026-08-11: Implemented. GitHub 9 → 13 operations, GitLab 7 → 9, catalogue 829 → 835.

## Notes

**Three reviewed gates asserted a premise C-30 had already invalidated, and were corrected rather
than relaxed.** `crates/connector-flux/tests/github_connector.rs` opened with *"nothing in this
pipeline percent-encodes a query value: the emitter interpolates it verbatim"* and enforced the only
rule that was safe under it — every query parameter is an integer. C-30 landed Flux 0.54's structured
`http.request(query: …)` map, so a scalar value now travels as a record field with RFC 3986 encoding
and the URL carries path data only. Verified on the emitted text before anything was changed.

"Integers only" was a **proxy** for the property that matters, and the proxy now excludes safe
parameters while proving nothing extra. The replacements are strictly stronger:

- every query parameter is a **scalar** — an array or object has no declared wire shape and C-30
  refuses one with `UnencodableQueryValue`;
- **no query value reaches the URL**, asserted on *every* operation rather than on four exempted ids.

The narrower parameter sets on the already-published reads are kept, and their comments now say why
they are kept: they are **compatibility** bounds on request bytes already in the catalogue, not
safety bounds. Widening them moves published bytes and is a separate reviewed change.

GitLab's numeric-project-id rule is untouched and is now precisely a *path* rule: C-30 encoded
queries, but `:id` is a path segment composed with `fmt`, so a namespaced `group/project` still
cannot travel.

`github-user-repo-list` omits `since` and `before`: they page by repository creation date and
interact with `sort`/`direction` in a way the vendor documents only in prose.
