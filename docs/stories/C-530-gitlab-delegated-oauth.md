---
id: C-530
title: "GitLab authenticates as the integration or on behalf of a user"
pillar: Connector
status: done
priority: 0
areas: [providers, connector-spec, tests]
note: "the first shipped OAuth2 connector — an org-wide static token or a per-user grant, declared as alternatives, with one operator-approved origin serving both the API and the OAuth endpoints of gitlab.com or a self-hosted instance"
---

# GitLab authenticates as the integration or on behalf of a user

## Goal

Support both deployments of the same connector: one credential provisioned once for a whole
organisation, or a grant each signed-in person completes for themselves.

## Acceptance

- [x] `gitlab.oauth_token` is declared beside `gitlab.token`, with an `[auth.oauth2]` block naming
      the `login` service, `authorization_code` and `refresh_token` grants, and
      `read_api`/`read_user`/`read_repository` scopes.
- [x] `default_auth` lists the two as **alternatives**, not as a pair. Every operation accepts either
      and requires only one; nothing in this repository chooses between them.
- [x] Both credentials declare `subject = "user"` — a GitLab personal access token acts as its
      creator, and a delegated OAuth token acts as the person who granted it. Neither is an
      integration identity.
- [x] The OAuth application is **operator** level (`oauth.client_id`, `oauth.client_secret`), derived
      from `binds` rather than authored, so an end user is never asked for the product's own secret.
- [x] `read_repository` is requested and the reason recorded: it is what lets the resulting token
      clone over HTTPS. Cloning is not a connector operation — git speaks its own wire protocol —
      but the credential a git client is handed is this one.
- [x] **gitlab.com and a self-hosted instance both work, from one question.** The `origin` field
      defaults to `https://gitlab.com` and, through [C-529](C-529-one-deployment-one-origin-question.md)'s
      `also_services`, fills the OAuth service too — so an approved self-managed origin moves the API
      and the token endpoint together, under one approval.
- [x] The `login` service declares no operations and must not: an authorize endpoint is a browser
      redirect and a token endpoint's response body *is* a credential, so neither is a connector
      operation.
- [x] `connectors/gitlab.flux` is byte-identical — no published request moved.

## Progress

- 2026-08-11: Implemented. GitLab is the first shipped connector to declare `[auth.oauth2]`.

## Notes

**Two reviewed gates recorded this as an open gap and named their own successors.** `auth_archetypes.rs`
asserted that *no* shipped provider declared `[auth.oauth2]`, so the operator level of the
configuration model was exercised only by fixtures; it said the replacement should assert "the form
OAuth generates: an operator-level client id and client secret". That is now what it asserts, over
the whole corpus, plus two properties it could not have anticipated: the grant must name a declared
service, and the credential must state its `subject`.

`services.rs` refused any service with no operations, because a build emits an empty module for one.
That is now "no service exists for nothing": an operation-less service is admitted when an
`[auth.oauth2]` block names it. The empty `gitlab-login.flux` is a real remaining wart — the module
has nothing to declare and should probably not be emitted at all.

What this does **not** do: mint tokens, refresh them, or store a user-scoped credential per
principal. The host runs the grant. `connector-secrets` still has no expiry, refresh, rotation or
revocation — and a GitLab OAuth access token expires in two hours by default, so that gap is now on
the critical path for any unattended fleet rather than a nicety.
