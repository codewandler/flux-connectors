---
id: C-52
title: Ship the GitHub connector
pillar: Spec
status: ready
priority: 3
design: docs/designs/provider-operation-inventory.md
epic: connectors-v1
areas: [providers, connector-spec]
note: bearer · path-and-body surface only · listing ops wait on C-30
---

# Ship the GitHub connector

## Goal
Add `providers/github.toml` and its generated artifacts, curated to the part of the GitHub REST API
that this pipeline can express honestly today: path parameters and JSON bodies, no query strings.

## Acceptance
- [ ] `providers/github.toml` is hand-authored, following the zendesk precedent. GitHub publishes
      `github/rest-api-description`, which is the `[spec]` pointer this file becomes once C-4 lands;
      record that in the header comment along with the operation set as the selection to reproduce.
- [ ] `base_url = "https://api.github.com"`, `vendor = "GitHub"`, `[[auth]]` with
      `scheme = "bearer"` over `GITHUB_TOKEN`, named by `default_auth`.
- [ ] A curated operation set of roughly five, each with `risk` and `idempotency`. Confirm against
      current vendor docs; the intended set is `github-repo-get` ·
      `github-issue-get` · `github-issue-create` · `github-issue-comment-add` · `github-pull-get`,
      all addressed by path parameters (`{owner}`, `{repo}`, `{issue_number}`, `{pull_number}`).
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested. C-30 is not
      implemented and the emitter would emit such a value unencoded — `state`, `labels` and `q` are
      exactly the injectable shapes `zendesk-ticket-search` already demonstrates. Listing and search
      operations are therefore out of scope here and named in Notes.
- [ ] **GitHub's required `Accept: application/vnd.github+json` header is either declared or reported
      as a schema gap.** If no field can express a constant, non-credential header, say so in the
      header comment and in this story's Progress, following the `SCHEMA GAP:` precedent in
      `providers/zendesk.toml` — do not smuggle it in as a parameter with a default the caller can
      overwrite unless that is genuinely what the schema means.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/github.flux` and
      `connectors/github.connector.toml`, committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point tests every
      existing provider passes.
- [ ] `crates/catalog/src/generated.rs` gains its `pub(crate) mod github;` line and its `PROVIDERS`
      entry in id order — `crates/catalog/tests/embedded_operations.rs` fails until it does.
- [ ] `http_hosts` is `api.github.com`, never widened; no credential value in any generated artifact.
- [ ] `github-issue-create` and `github-issue-comment-add` write to a public surface; their `risk`
      says so.

## Progress
- Not started. Filed 2026-07-30 under "ship up to 3 connectors, popular and useful".

## Notes
- **Deliberately excluded pending C-30**: `github-issue-list` (`state`, `labels`, `assignee`),
  `github-pull-list` and every search endpoint. They are the most-wanted GitHub operations and they
  are precisely the ones the query-encoding gap makes unsafe — which makes this connector a strong
  second argument for C-30 and for flux's structured `query` map.
- GitHub Apps and fine-grained tokens both present as `Authorization: Bearer <token>`, so one auth
  method covers both; the token *type* is operator config, not connector shape.
- **Still cannot make a live call** — the `$auth` whole-value-replacement gap
  (`docs/designs/auth-seam.md`) applies here as to every connector.
