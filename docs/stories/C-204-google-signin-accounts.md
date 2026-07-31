---
id: C-204
title: "Google sign-in, accounts and sessions"
pillar: Bridge
status: ready
priority: 3
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, host]
note: "the principal. Until this lands the service has no answer to 'who is asking', which is the question connectors-proxy.md says a credential-injecting service must answer before it injects anything"
---

# Google sign-in, accounts and sessions

## Goal

Give the service a principal: a person signs in with Google, gets an account, and every subsequent
request resolves to exactly one tenant on the strength of that session.

## Why it is not optional, and not last

[connectors-proxy.md](../designs/connectors-proxy.md)'s rejection turned on the deputy problem — a
service that adds authority a caller does not have. The answer to it is not "the service is
authenticated", which that design already dismissed as insufficient; it is **the caller is the
principal whose credential is being used**. That equation is what this story establishes, and every
credential the service holds before it lands is held for nobody in particular.

## Acceptance

- [ ] Sign-in with Google via OIDC authorization-code + PKCE. `id_token` is verified — signature
      against Google's JWKS, `iss`, `aud`, `exp`, and `nonce`. A test asserts each check by feeding a
      token that fails exactly one of them.
- [ ] An account is created on first sign-in and keyed by the OIDC `sub`, **not** by email. Email is
      mutable and reassignable; `sub` is the stable subject identifier, and keying on email is how one
      person inherits another's credentials.
- [ ] A session is an opaque server-side token, `HttpOnly`, `Secure`, `SameSite=Lax`, with an
      expiry and a revocation path. No credential material and no tenant secret is ever in a cookie.
- [ ] **Failing-first:** a test proving a request carrying tenant B's identifier but tenant A's
      session resolves to tenant **A** — the tenant comes from the session, never from the request
      body, the path, or a header.
- [ ] Sign-out revokes server-side, so a stolen cookie stops working.
- [ ] The Google client secret is resolved from the environment or the secret store, never from a
      provider TOML, a generated artifact, or a committed file.

## Notes

- `codewandler-flux-credentials` (0.42.1) already implements PKCE generation, the S256 authorize-URL
  builder and the form-encoded token exchange, and flux's CLI drives it three times over
  (`login_claude`, `login_codex`, `login_plugin`). **Reuse rather than write a fourth** — a fourth
  PKCE implementation in this ecosystem is how two drift on a security-relevant detail. Its 0.42
  version against this workspace's 0.41 pin is the open question; see
  [C-202](C-202-flux-web-egress.md)'s skew note.
- Google's OIDC sign-in and the `google` **connector** are different things and must not be conflated.
  Signing in proves who the operator is; it does not mint a token for `google-gmail-message-get`.
  That is [C-207](C-207-oauth2-connect-flow.md), with different scopes and a different consent screen.
