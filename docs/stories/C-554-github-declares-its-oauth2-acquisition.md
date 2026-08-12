---
id: C-554
title: "GitHub declares its OAuth2 acquisition"
pillar: Connector
status: done
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

- [x] The declaration follows the gitlab pattern: an auth-host service (base `https://github.com`)
      distinct from the API service, with the OAuth2 spec's endpoint naming it; authorize path
      `/login/oauth/authorize`, token path `/login/oauth/access_token`, both verified against
      GitHub's published OAuth documentation and recorded with the verification source in the
      provider file's comments.
- [x] Grants are declared honestly: `authorization_code` (and `refresh_token` ONLY if the
      declaration targets GitHub-App-style expiring tokens — decide deliberately, record why; a
      classic OAuth app token does not refresh, and declaring a grant the vendor will not honour
      is the downgrade-shaped lie the loader should refuse if it can).
- [x] Scopes are the minimal set the connector's declared operations need, derived from the
      operations' documented requirements, not a grab-bag; each scope's reason is a comment.
- [x] No registration value: `client_id`/secret/redirect remain deployment configuration
      requirements per the amended Decision 0022 — the loader refuses a value (C-536 rule).
- [x] The scoped provider gate is green (`build --provider github`, `diff --provider github`,
      workspace build/test/clippy/fmt with the documented expected staleness reds only), and the
      canonical document carries the complete OAuth2Spec — quote
      `catalog/github.catalog.json`'s auth block in the report.
- [x] Exchange-side composability is sanity-checked from THIS repo: the declaration's endpoint
      service base URL is non-templated (or templated only with a declared default), so a startup
      composition can resolve it — the X-154 `NoDeclaredDefault` rule is the consumer contract.

## Progress

- 2026-08-12: Filed by the cross-repo coordinator for the exchange/autodev OAuth login goal.

- 2026-08-12: Implemented on `impl/C-554` (`49e9f45b`), merged `a5c6ab4f`. Endpoints verified
  against docs.github.com (authorize `github.com/login/oauth/authorize`, token
  `/login/oauth/access_token`; the auth host is github.com, distinct from api.github.com);
  grants `authorization_code` only, the classic-OAuth-app model chosen deliberately because it
  takes scopes and issues no refresh token (the GitHub App model is the recorded reversible
  alternative — delete scopes, do not add a grant); scopes `["repo","read:org"]`, each the
  vendor's documented floor with three declines recorded. The new `github-login` auth-host
  service takes the catalogue to 68 services / 1169 artifacts (counts updated at integration).
  A per-provider evidence test (`resend_connector.rs`'s predecessors list) was moved off github
  onto openrouter, a verified predecessor, because github growing a config surface falsified it —
  a legitimate per-provider-test correction, not staleness.

## Notes

- Write set: `providers/github.toml` + per-provider artifacts. A provider story; never touches
  whole-catalogue artifacts, COVERED_FLOOR, or the fence.
- gitlab.toml's `login` service + `oauth_token` credential is the worked example; read it first.
