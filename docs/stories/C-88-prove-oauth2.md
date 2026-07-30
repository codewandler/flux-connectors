---
id: C-88
title: Prove OAuth2 on one provider — the operator level is currently unexercised
pillar: Spec
status: ready
priority: 4
design: docs/designs/connector-configuration.md
epic: connector-config
areas: [connector-spec, providers]
note: "OAuth2Spec is a landed type NO shipped provider uses, so half the configuration model is proven only by a fixture. tests/auth_archetypes.rs asserts that gap and fails the day this lands"
---

# Prove OAuth2 on one provider — the operator level is currently unexercised

## Goal
Give the operator level of the configuration model a real provider behind it. `OAuth2Spec` has existed
since C-2 and **no shipped provider declares one**, so `oauth.client_id` / `oauth.client_secret` are
exercised by a fixture and nothing else.

## Acceptance
- [ ] One provider — Slack or HubSpot — declares an `[auth.oauth2]` credential as a **second
      alternative** beside its existing token credential, so the OR structure a UI renders as tabs is
      exercised at the same time.
- [ ] Its `[[config]]` declares the operator-level `client_id` and `client_secret`, and the
      connection-level half becomes a consent step rather than a pasted token.
- [ ] `crates/connector-spec/tests/auth_archetypes.rs::no_shipped_provider_exercises_oauth_yet` is
      **replaced** — it exists precisely to fail here, and its message says what to replace it with:
      an assertion about the form OAuth generates.
- [ ] The scopes the grant requests are declared rather than living in a credential `description`
      string. Depends on or lands with [C-67](C-67-required-scopes.md).
- [ ] A test asserts the two alternatives are genuinely alternatives — either authenticates every
      operation — rather than one being a subset of the other.
- [ ] The choice of provider is recorded with its reason. Slack is the natural candidate for a hosted
      product (OAuth v2 app install is how a SaaS connects a workspace), but its TOML currently states
      the opposite — *"flux is handed an already-minted token through the environment and runs no
      grant"* — so that comment must be amended rather than contradicted silently.

## Progress
- Not started. Filed 2026-07-30 with [C-86](C-86-connector-configuration-epic.md).

## Notes
- **Sequence with [C-89](C-89-hosted-oauth-redirect.md).** A hosted OAuth flow needs a callback URL
  that `OAuthRedirect { port, path }` cannot express, so proving OAuth on a provider without closing
  that gap proves only the loopback case.
- This is also what makes [C-21](C-21-effectful-acquisition.md)'s effectful-acquisition work
  testable against something real.
