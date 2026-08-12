---
id: C-554
title: "GitHub declares its OAuth2 acquisition"
pillar: Connector
status: ready
priority: 1
epic: catalog-artifact
areas: [providers]
note: "The cross-repo login goal (exchange/autodev) needs github, gitlab and anthropic composable via OAuth2; gitlab already declares, github carries only a bearer token. GitHub's auth host (github.com) differs from its API host (api.github.com) — the gitlab login-service pattern is the template"
---

# GitHub declares its OAuth2 acquisition

## Goal

`providers/github.toml` declares the OAuth2 authorization-code acquisition for its existing
`github.token` credential slot (or a sibling credential, deliberately chosen), so a host can
compose GitHub's authorize URL from the artifact alone — the same way gitlab composes today.
Exchange's composition path (X-154) reads exactly this declaration; autodev's GitHub login is the
consumer.

## Acceptance

- [ ] The declaration follows the gitlab pattern: an auth-host service (base `https://github.com`)
      distinct from the API service, with the OAuth2 spec's endpoint naming it; authorize path
      `/login/oauth/authorize`, token path `/login/oauth/access_token`, both verified against
      GitHub's published OAuth documentation and recorded with the verification source in the
      provider file's comments.
- [ ] Grants are declared honestly: `authorization_code` (and `refresh_token` ONLY if the
      declaration targets GitHub-App-style expiring tokens — decide deliberately, record why; a
      classic OAuth app token does not refresh, and declaring a grant the vendor will not honour
      is the downgrade-shaped lie the loader should refuse if it can).
- [ ] Scopes are the minimal set the connector's declared operations need, derived from the
      operations' documented requirements, not a grab-bag; each scope's reason is a comment.
- [ ] No registration value: `client_id`/secret/redirect remain deployment configuration
      requirements per the amended Decision 0022 — the loader refuses a value (C-536 rule).
- [ ] The scoped provider gate is green (`build --provider github`, `diff --provider github`,
      workspace build/test/clippy/fmt with the documented expected staleness reds only), and the
      canonical document carries the complete OAuth2Spec — quote
      `catalog/github.catalog.json`'s auth block in the report.
- [ ] Exchange-side composability is sanity-checked from THIS repo: the declaration's endpoint
      service base URL is non-templated (or templated only with a declared default), so a startup
      composition can resolve it — the X-154 `NoDeclaredDefault` rule is the consumer contract.

## Progress

- 2026-08-12: Filed by the cross-repo coordinator for the exchange/autodev OAuth login goal.

## Notes

- Write set: `providers/github.toml` + per-provider artifacts. A provider story; never touches
  whole-catalogue artifacts, COVERED_FLOOR, or the fence.
- gitlab.toml's `login` service + `oauth_token` credential is the worked example; read it first.
